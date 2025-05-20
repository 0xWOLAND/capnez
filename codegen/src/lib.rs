use anyhow::{Context, Result};
use std::{fs, path::PathBuf, env, collections::{HashMap, HashSet}};
use walkdir::WalkDir;
use syn::{parse_file, Item, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument, Attribute, ItemTrait, Meta};

#[derive(Clone)]
enum CapnpType {
    Text, UInt32, UInt64, Float32, Float64, Bool, Bytes,
    List(Box<CapnpType>),
    Optional(Box<CapnpType>),
    Struct(String),
}

impl std::fmt::Display for CapnpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "Text"),
            Self::UInt32 => write!(f, "UInt32"),
            Self::UInt64 => write!(f, "UInt64"),
            Self::Float32 => write!(f, "Float32"),
            Self::Float64 => write!(f, "Float64"),
            Self::Bool => write!(f, "Bool"),
            Self::List(inner) => write!(f, "List({})", inner),
            Self::Optional(inner) => write!(f, "union {{\n  value @0 :{};\n  none @1 :Void;\n}}", inner),
            Self::Struct(name) => write!(f, "{}", name),
            Self::Bytes => write!(f, "List(UInt8)"),
        }
    }
}

#[derive(Clone)]
struct CapnpStruct {
    name: String,
    fields: Vec<(String, usize, CapnpType)>,
    has_serde: bool,
    is_bytes: bool,
}

impl CapnpStruct {
    fn dependencies(&self) -> HashSet<String> {
        self.fields.iter()
            .filter_map(|(_, _, ty)| match ty {
                CapnpType::Struct(name) => Some(name.clone()),
                CapnpType::List(inner) | CapnpType::Optional(inner) => match &**inner {
                    CapnpType::Struct(name) => Some(name.clone()),
                    _ => None
                },
                _ => None
            })
            .collect()
    }
}

#[derive(Clone)]
struct CapnpInterface {
    name: String,
    methods: Vec<(String, Vec<(String, CapnpType)>, Option<CapnpType>)>,
}

#[derive(Default)]
struct StructRegistry(HashMap<String, (bool, bool)>);

impl StructRegistry {
    fn register_serde_struct(&mut self, name: &str) { 
        let entry = self.0.entry(name.to_string()).or_insert((false, false));
        entry.1 = true;
    }
    fn register_capnp_struct(&mut self, name: &str) {
        let entry = self.0.entry(name.to_string()).or_insert((false, false));
        entry.0 = true;
    }
    fn is_serde_struct(&self, name: &str) -> bool { 
        self.0.get(name).map_or(false, |(_, serde)| *serde) 
    }
    fn is_capnp_struct(&self, name: &str) -> bool {
        self.0.get(name).map_or(false, |(capnp, _)| *capnp)
    }
}

fn has_attrs(attrs: &[Attribute]) -> (bool, bool) {
    attrs.iter().fold((false, false), |(capnp, serde), attr| {
        let ident = attr.path().segments.last().map(|s| s.ident.to_string());
        match ident.as_deref() {
            Some("capnp") => (true, serde),
            Some("serde") => (capnp, true),
            Some("derive") => {
                if let syn::Meta::List(list) = &attr.meta {
                    let has_serde = list.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
                        .unwrap_or_default()
                        .iter()
                        .any(|meta| matches!(meta, syn::Meta::Path(p) if p.segments.last().map_or(false, |s| s.ident == "Serialize" || s.ident == "Deserialize")));
                    (capnp, serde || has_serde)
                } else { (capnp, serde) }
            }
            _ => (capnp, serde)
        }
    })
}

fn map_ty(ty: &Type, registry: &StructRegistry) -> CapnpType {
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let id = p.path.segments.last().unwrap().ident.to_string();
            match id.as_str() {
                "String" => CapnpType::Text,
                "u32" => CapnpType::UInt32,
                "u64" => CapnpType::UInt64,
                "f32" => CapnpType::Float32,
                "f64" => CapnpType::Float64,
                "bool" => CapnpType::Bool,
                "Option" => CapnpType::Optional(Box::new(extract_generic_ty(p, registry))),
                "Vec" => CapnpType::List(Box::new(extract_generic_ty(p, registry))),
                name => {
                    let pascal_name = name.split('_').map(|w| {
                        let mut c = w.chars();
                        c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
                    }).collect::<String>();
                    if registry.is_serde_struct(&pascal_name) && !registry.is_capnp_struct(&pascal_name) {
                        CapnpType::Bytes
                    } else {
                        CapnpType::Struct(pascal_name)
                    }
                }
            }
        }
        Type::Array(a) => CapnpType::List(Box::new(map_ty(&a.elem, registry))),
        _ => panic!("Unsupported type"),
    }
}

fn extract_generic_ty(p: &syn::TypePath, registry: &StructRegistry) -> CapnpType {
    match &p.path.segments[0].arguments {
        PathArguments::AngleBracketed(args) => args.args.first()
            .and_then(|arg| match arg {
                GenericArgument::Type(inner_ty) => Some(map_ty(inner_ty, registry)),
                _ => None
            })
            .unwrap_or_else(|| panic!("Generic type must have a type parameter")),
        _ => panic!("Generic type must have angle bracketed arguments"),
    }
}

fn mk_struct(input: &DeriveInput, has_serde: bool, registry: &mut StructRegistry) -> CapnpStruct {
    let name = input.ident.to_string().split('_').map(|w| {
        let mut c = w.chars();
        c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
    }).collect::<String>();
    
    if has_serde {
        registry.register_serde_struct(&name);
    }
    registry.register_capnp_struct(&name);

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => n.named.iter().enumerate().map(|(i, f)| {
                let field_name = f.ident.as_ref().unwrap().to_string();
                let camel_name = field_name.split('_').enumerate().map(|(i, w)| {
                    let mut c = w.chars();
                    if i == 0 { c.next().map_or(String::new(), |f| f.to_lowercase().chain(c).collect()) }
                    else { c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect()) }
                }).collect::<String>();
                (camel_name, i, map_ty(&f.ty, registry))
            }).collect(),
            _ => panic!("Only named structs are supported"),
        },
        _ => panic!("Only structs are supported"),
    };
    CapnpStruct { name, fields, has_serde, is_bytes: false }
}

fn mk_interface(input: &ItemTrait) -> CapnpInterface {
    let name = input.ident.to_string().split('_').map(|w| {
        let mut c = w.chars();
        c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
    }).collect::<String>();

    let methods = input.items.iter().filter_map(|item| {
        if let syn::TraitItem::Fn(method) = item {
            let name = method.sig.ident.to_string().split('_').enumerate().map(|(i, w)| {
                let mut c = w.chars();
                if i == 0 { c.next().map_or(String::new(), |f| f.to_lowercase().chain(c).collect()) }
                else { c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect()) }
            }).collect::<String>();

            let params = method.sig.inputs.iter().filter_map(|arg| {
                if let syn::FnArg::Typed(pat_type) = arg {
                    if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                        let param_name = pat_ident.ident.to_string().split('_').enumerate().map(|(i, w)| {
                            let mut c = w.chars();
                            if i == 0 { c.next().map_or(String::new(), |f| f.to_lowercase().chain(c).collect()) }
                            else { c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect()) }
                        }).collect::<String>();
                        Some((param_name, map_ty(&pat_type.ty, &StructRegistry::default())))
                    } else { None }
                } else { None }
            }).collect();

            let ret = match &method.sig.output {
                syn::ReturnType::Type(_, ty) => Some(map_ty(&ty, &StructRegistry::default())),
                syn::ReturnType::Default => None,
            };
            Some((name, params, ret))
        } else { None }
    }).collect();

    CapnpInterface { name, methods }
}

fn topo_sort(structs: &[CapnpStruct]) -> Vec<usize> {
    let mut visited = HashSet::new();
    let mut temp = HashSet::new();
    let mut order = Vec::new();
    
    fn visit(i: usize, structs: &[CapnpStruct], visited: &mut HashSet<usize>, 
             temp: &mut HashSet<usize>, order: &mut Vec<usize>) -> bool {
        if temp.contains(&i) { return false; }
        if visited.contains(&i) { return true; }
        
        temp.insert(i);
        for dep in structs[i].dependencies() {
            if let Some(j) = structs.iter().position(|s| s.name == dep) {
                if !visit(j, structs, visited, temp, order) { return false; }
            }
        }
        temp.remove(&i);
        visited.insert(i);
        order.push(i);
        true
    }
    
    for i in 0..structs.len() {
        if !visited.contains(&i) && !visit(i, structs, &mut visited, &mut temp, &mut order) {
            panic!("Circular dependency detected in struct definitions");
        }
    }
    order.reverse();
    order
}

fn collect_structs(file: &syn::File, registry: &mut StructRegistry) -> Vec<CapnpStruct> {
    // First pass: register all serde structs
    for item in &file.items {
        if let Item::Struct(s) = item {
            let (_, has_serde) = has_attrs(&s.attrs);
            if has_serde {
                let name = s.ident.to_string().split('_').map(|w| {
                    let mut c = w.chars();
                    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
                }).collect::<String>();
                registry.register_serde_struct(&name);
            }
        }
    }

    // Second pass: collect capnp structs
    let mut structs = Vec::new();
    for item in &file.items {
        if let Item::Struct(s) = item {
            let (has_capnp, has_serde) = has_attrs(&s.attrs);
            let name = s.ident.to_string().split('_').map(|w| {
                let mut c = w.chars();
                c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
            }).collect::<String>();
            if has_serde {
                registry.register_serde_struct(&name);
            }
            if has_capnp {
                registry.register_capnp_struct(&name);
            }
            if has_capnp {
                let input = DeriveInput {
                    attrs: s.attrs.clone(),
                    vis: s.vis.clone(),
                    ident: s.ident.clone(),
                    generics: s.generics.clone(),
                    data: Data::Struct(syn::DataStruct {
                        struct_token: s.struct_token,
                        fields: s.fields.clone(),
                        semi_token: s.semi_token,
                    }),
                };
                structs.push(mk_struct(&input, has_serde, registry));
            }
        }
    }
    structs
}

pub fn generate_schema() -> Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let output = out_dir.join("generated");
    fs::create_dir_all(&output)?;
    
    let mut structs = Vec::new();
    let mut interfaces = Vec::new();
    let mut registry = StructRegistry::default();
    
    // First pass: collect all files to register serde structs
    let files: Vec<_> = WalkDir::new(manifest_dir.join("src"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        .collect();

    // First pass: register all serde structs
    for entry in &files {
        let content = fs::read_to_string(entry.path())
            .with_context(|| format!("Failed to read {}", entry.path().display()))?;
            
        let file = parse_file(&content)
            .with_context(|| format!("Failed to parse {}", entry.path().display()))?;
            
        // Register serde structs first
        for item in &file.items {
            if let Item::Struct(s) = item {
                let (has_capnp, has_serde) = has_attrs(&s.attrs);
                let name = s.ident.to_string().split('_').map(|w| {
                    let mut c = w.chars();
                    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
                }).collect::<String>();
                if has_serde {
                    registry.register_serde_struct(&name);
                }
                if has_capnp {
                    registry.register_capnp_struct(&name);
                }
            }
        }
    }

    // Second pass: collect capnp structs and interfaces
    for entry in files {
        let content = fs::read_to_string(entry.path())
            .with_context(|| format!("Failed to read {}", entry.path().display()))?;
            
        let file = parse_file(&content)
            .with_context(|| format!("Failed to parse {}", entry.path().display()))?;
            
        structs.extend(collect_structs(&file, &mut registry));
        
        for item in file.items {
            if let Item::Trait(t) = item {
                let (has_capnp, _) = has_attrs(&t.attrs);
                if has_capnp { interfaces.push(mk_interface(&t)); }
            }
        }
    }

    let mut schema = String::from("@0xabcdefabcdefabcdef;\n\n");
    
    // Sort structs topologically
    let order = topo_sort(&structs);
    for &i in &order {
        let s = &structs[i];
        schema.push_str(&format!("struct {} {{\n", s.name));
        for (name, id, ty) in &s.fields {
            schema.push_str(&format!("  {} @{} :{};\n", name, id, ty));
        }
        schema.push_str("}\n\n");
    }
    
    for i in &interfaces {
        schema.push_str(&format!("interface {} {{\n", i.name));
        for (name, params, ret) in &i.methods {
            schema.push_str(&format!("  {} @0 (", name));
            for (i, (pname, pty)) in params.iter().enumerate() {
                if i > 0 { schema.push_str(", "); }
                schema.push_str(&format!("{} :{}", pname, pty));
            }
            schema.push_str(")");
            if let Some(ret) = ret { schema.push_str(&format!(" -> {}", ret)); }
            schema.push_str(";\n");
        }
        schema.push_str("}\n\n");
    }
    
    let schema_path = output.join("schema.capnp");
    fs::write(&schema_path, schema)?;
    
    capnpc::CompilerCommand::new()
        .file(&schema_path)
        .output_path(&output)
        .src_prefix(&output)
        .run()
        .context("Failed to compile Cap'n Proto schema")?;

    let capnp_path = output.join("schema_capnp.rs");
    let mut capnp_code = fs::read_to_string(&capnp_path)
        .context("Failed to read generated Cap'n Proto code")?;

    // Only add serde imports if any struct has serde
    if structs.iter().any(|s| s.has_serde) {
        capnp_code = "#[cfg(feature = \"serde\")]\nuse serde::{Serialize, Deserialize};\n\n".to_string() + &capnp_code;
    }

    for s in &structs {
        if s.has_serde {
            let derive = format!("#[cfg_attr(feature = \"serde\", derive(Serialize, Deserialize))]\n");
            capnp_code = capnp_code.replace(&format!("pub struct {}", s.name), &format!("{}\npub struct {}", derive, s.name));
        }
    }

    fs::write(&capnp_path, capnp_code)?;
    Ok(())
}

#[macro_export]
macro_rules! capnp_include {
    () => {
        pub mod schema_capnp {
            include!(concat!(env!("OUT_DIR"), "/generated/schema_capnp.rs"));
        }
    };
}