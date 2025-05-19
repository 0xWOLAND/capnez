use anyhow::{Context, Result};
use std::{fs, path::PathBuf, collections::HashSet, env};
use walkdir::WalkDir;
use syn::{parse_file, Item, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument, ItemTrait, Attribute};

#[derive(Clone, Copy)]
enum SerializationType {
    Capnp,
    Serde,
    Both,
}

#[derive(Clone)]
enum CapnpType {
    Primitive(&'static str),
    Struct(String),
    List(Box<CapnpType>),
    Enum(String),
    Optional(Box<CapnpType>),
    SerdeOnly(String), // For types that only exist in Serde
}

impl std::fmt::Display for CapnpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Primitive(p) => write!(f, "{}", p),
            Self::Struct(n) => write!(f, "{}", n),
            Self::List(inner) => write!(f, "List({})", inner),
            Self::Enum(n) => write!(f, "{}", n),
            Self::Optional(inner) => write!(f, "union {{\n  value @0 :{};\n  none @1 :Void;\n}}", inner),
            Self::SerdeOnly(n) => write!(f, "{}", n),
        }
    }
}

#[derive(Clone)]
struct CapnpStruct {
    name: String,
    fields: Vec<(String, usize, CapnpType)>,
    has_serde: bool,
}

#[derive(Clone)]
struct CapnpEnum {
    name: String,
    variants: Vec<(String, Option<CapnpType>)>,
    has_serde: bool,
}

#[derive(Clone)]
struct CapnpInterface {
    name: String,
    methods: Vec<(String, Vec<(String, CapnpType)>, Option<CapnpType>)>,
}

fn has_serialization_attr(attrs: &[Attribute]) -> (bool, bool) {
    let mut has_capnp = false;
    let mut has_serde = false;
    
    for attr in attrs {
        if let Some(seg) = attr.path().segments.last() {
            match seg.ident.to_string().as_str() {
                "capnp" => has_capnp = true,
                "serde" => has_serde = true,
                "derive" => {
                    if let syn::Meta::List(list) = &attr.meta {
                        for nested in list.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated).unwrap_or_default() {
                            if let syn::Meta::Path(path) = nested {
                                if let Some(seg) = path.segments.last() {
                                    match seg.ident.to_string().as_str() {
                                        "Serialize" | "Deserialize" => has_serde = true,
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    (has_capnp, has_serde)
}

fn map_ty(ty: &Type, ser_type: SerializationType) -> CapnpType {
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let id = p.path.segments.last().unwrap().ident.to_string();
            match id.as_str() {
                "String" => CapnpType::Primitive("Text"),
                "u32" => CapnpType::Primitive("UInt32"),
                "u64" => CapnpType::Primitive("UInt64"),
                "bool" => CapnpType::Primitive("Bool"),
                "Option" => CapnpType::Optional(Box::new(extract_generic_ty(p, ser_type.clone()))),
                "Vec" => CapnpType::List(Box::new(extract_generic_ty(p, ser_type.clone()))),
                "HashMap" | "BTreeMap" => {
                    if matches!(ser_type, SerializationType::Serde) {
                        CapnpType::SerdeOnly(id)
                    } else {
                        panic!("Map types are only supported with Serde serialization")
                    }
                }
                "HashSet" | "BTreeSet" => {
                    if matches!(ser_type, SerializationType::Serde) {
                        CapnpType::SerdeOnly(id)
                    } else {
                        panic!("Set types are only supported with Serde serialization")
                    }
                }
                name => CapnpType::Struct(name.to_string())
            }
        }
        Type::Array(a) => CapnpType::List(Box::new(map_ty(&a.elem, ser_type))),
        _ => panic!("Unsupported type"),
    }
}

fn extract_generic_ty(p: &syn::TypePath, ser_type: SerializationType) -> CapnpType {
    match &p.path.segments[0].arguments {
        PathArguments::AngleBracketed(args) => args.args.first()
            .and_then(|arg| match arg {
                GenericArgument::Type(inner_ty) => Some(map_ty(inner_ty, ser_type)),
                _ => None
            })
            .unwrap_or_else(|| panic!("Generic type must have a type parameter")),
        _ => panic!("Generic type must have angle bracketed arguments")
    }
}

fn to_camel_case(s: &str, is_type: bool) -> String {
    let mut result = String::new();
    let mut capitalize = is_type; // Only capitalize first letter for types
    
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_ascii_uppercase());
            capitalize = false;
        } else {
            result.push(c);
        }
    }
    result
}

fn mk_struct(input: &DeriveInput, ser_type: SerializationType) -> CapnpStruct {
    let name = to_camel_case(&input.ident.to_string(), true);
    let has_serde = matches!(ser_type, SerializationType::Serde | SerializationType::Both);
    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => n.named.iter()
                .enumerate()
                .map(|(i, f)| {
                    let field_name = f.ident.as_ref().unwrap().to_string();
                    let serde_rename = f.attrs.iter()
                        .find_map(|attr| {
                            if let syn::Meta::NameValue(nv) = &attr.meta {
                                if let Some(seg) = attr.path().segments.last() {
                                    if seg.ident == "serde" {
                                        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = &nv.value {
                                            return Some(s.value());
                                        }
                                    }
                                }
                            }
                            None
                        })
                        .unwrap_or(field_name.clone());
                    (to_camel_case(&serde_rename, false), i, map_ty(&f.ty, ser_type))
                })
                .collect(),
            _ => panic!("Only named structs are supported")
        },
        _ => panic!("Only structs are supported"),
    };
    CapnpStruct { name, fields, has_serde }
}

fn mk_enum(input: &DeriveInput, data: &syn::DataEnum, ser_type: SerializationType) -> CapnpEnum {
    let name = to_camel_case(&input.ident.to_string(), true);
    let has_serde = matches!(ser_type, SerializationType::Serde | SerializationType::Both);
    let variants = data.variants.iter()
        .map(|v| {
            let ty = match &v.fields {
                syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => 
                    Some(map_ty(&fields.unnamed[0].ty, ser_type)),
                syn::Fields::Unnamed(_) => panic!("Enum variants must have exactly one unnamed field"),
                _ => None,
            };
            (to_camel_case(&v.ident.to_string(), false), ty)
        })
        .collect();
    CapnpEnum { name, variants, has_serde }
}

fn mk_interface(input: &ItemTrait) -> CapnpInterface {
    let name = to_camel_case(&input.ident.to_string(), true);
    let methods = input.items.iter()
        .filter_map(|item| {
            if let syn::TraitItem::Fn(method) = item {
                let name = to_camel_case(&method.sig.ident.to_string(), false);
                let params = method.sig.inputs.iter()
                    .filter_map(|arg| {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                Some((to_camel_case(&pat_ident.ident.to_string(), false), map_ty(&pat_type.ty, SerializationType::Capnp)))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
                let ret = match &method.sig.output {
                    syn::ReturnType::Type(_, ty) => Some(map_ty(&ty, SerializationType::Capnp)),
                    syn::ReturnType::Default => None,
                };
                Some((name, params, ret))
            } else {
                None
            }
        })
        .collect();
    CapnpInterface { name, methods }
}

fn sort_deps<'a>(items: &'a [CapnpStruct]) -> Vec<&'a CapnpStruct> {
    let mut seen = HashSet::new();
    let mut order = Vec::new();
    
    fn visit<'b>(s: &'b CapnpStruct, items: &'b [CapnpStruct], seen: &mut HashSet<String>, order: &mut Vec<&'b CapnpStruct>) {
        if !seen.insert(s.name.clone()) { return; }
        
        for (_, _, ty) in &s.fields {
            if let Some(name) = get_struct_name(ty) {
                if let Some(dep) = items.iter().find(|x| x.name == name) {
                    visit(dep, items, seen, order);
                }
            }
        }
        order.push(s);
    }
    
    for s in items {
        visit(s, items, &mut seen, &mut order);
    }
    order
}

fn get_struct_name(ty: &CapnpType) -> Option<String> {
    match ty {
        CapnpType::Struct(name) => Some(name.clone()),
        CapnpType::List(inner) => {
            let mut inner_ty = inner.as_ref();
            while let CapnpType::List(next) = inner_ty {
                inner_ty = next.as_ref();
            }
            if let CapnpType::Struct(name) = inner_ty {
                Some(name.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Generate Cap'n Proto schema from Rust source files
/// 
/// The schema will be generated in the target directory under `generated/schema.capnp`
/// 
/// # Returns
/// 
/// Returns `Result<()>` indicating success or failure
pub fn generate_schema() -> Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    
    let input = manifest_dir.join("src");
    let output = out_dir.join("generated");
    
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut interfaces = Vec::new();
    
    // Walk through all .rs files
    for entry in WalkDir::new(&input)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        let content = fs::read_to_string(entry.path())
            .with_context(|| format!("Failed to read {}", entry.path().display()))?;
            
        let file = parse_file(&content)
            .with_context(|| format!("Failed to parse {}", entry.path().display()))?;
            
        for item in file.items {
            match item {
                Item::Struct(s) => {
                    let (has_capnp, has_serde) = has_serialization_attr(&s.attrs);
                    if has_capnp || has_serde {
                        let ser_type = match (has_capnp, has_serde) {
                            (true, true) => SerializationType::Both,
                            (true, false) => SerializationType::Capnp,
                            (false, true) => SerializationType::Serde,
                            _ => continue,
                        };
                        let input = DeriveInput {
                            attrs: s.attrs,
                            vis: s.vis,
                            ident: s.ident,
                            generics: s.generics,
                            data: Data::Struct(syn::DataStruct {
                                struct_token: s.struct_token,
                                fields: s.fields,
                                semi_token: s.semi_token,
                            }),
                        };
                        structs.push(mk_struct(&input, ser_type));
                    }
                }
                Item::Enum(e) => {
                    let (has_capnp, has_serde) = has_serialization_attr(&e.attrs);
                    if has_capnp || has_serde {
                        let ser_type = match (has_capnp, has_serde) {
                            (true, true) => SerializationType::Both,
                            (true, false) => SerializationType::Capnp,
                            (false, true) => SerializationType::Serde,
                            _ => continue,
                        };
                        let input = DeriveInput {
                            attrs: e.attrs,
                            vis: e.vis,
                            ident: e.ident,
                            generics: e.generics,
                            data: Data::Enum(syn::DataEnum {
                                enum_token: e.enum_token,
                                brace_token: e.brace_token,
                                variants: e.variants.clone(),
                            }),
                        };
                        enums.push(mk_enum(&input, &syn::DataEnum {
                            enum_token: e.enum_token,
                            brace_token: e.brace_token,
                            variants: e.variants,
                        }, ser_type));
                    }
                }
                Item::Trait(t) => {
                    let (has_capnp, _) = has_serialization_attr(&t.attrs);
                    if has_capnp {
                        interfaces.push(mk_interface(&t));
                    }
                }
                _ => {}
            }
        }
    }

    // Create output directory if it doesn't exist
    fs::create_dir_all(&output)?;

    // Generate the Cap'n Proto schema
    let mut schema = String::from("@0xabcdefabcdefabcdef;\n\n");
    
    // Write enums first
    for e in &enums {
        if e.variants.iter().any(|(_, ty)| ty.is_some()) {
            schema.push_str(&format!("struct {} {{\n", e.name));
            for (i, (name, ty)) in e.variants.iter().enumerate() {
                let ty = ty.as_ref().map_or("Void".to_string(), |t| t.to_string());
                schema.push_str(&format!("  {} @{} :{};\n", name, i, ty));
            }
            schema.push_str("}\n\n");
        } else {
            schema.push_str(&format!("enum {} {{\n", e.name));
            for (i, (name, _)) in e.variants.iter().enumerate() {
                schema.push_str(&format!("  {} @{};\n", name, i));
            }
            schema.push_str("}\n\n");
        }
    }
    
    // Then write structs in dependency order
    for s in sort_deps(&structs) {
        schema.push_str(&format!("struct {} {{\n", s.name));
        for (name, id, ty) in &s.fields {
            schema.push_str(&format!("  {} @{} :{};\n", name, id, ty));
        }
        schema.push_str("}\n\n");
    }
    
    // Finally write interfaces
    for i in &interfaces {
        schema.push_str(&format!("interface {} {{\n", i.name));
        for (name, params, ret) in &i.methods {
            schema.push_str(&format!("  {} @0 (", name));
            for (i, (pname, pty)) in params.iter().enumerate() {
                if i > 0 { schema.push_str(", "); }
                schema.push_str(&format!("{} :{}", pname, pty));
            }
            schema.push_str(")");
            if let Some(ret) = ret {
                schema.push_str(&format!(" -> {}", ret));
            }
            schema.push_str(";\n");
        }
        schema.push_str("}\n\n");
    }
    
    // Write the schema to a .capnp file
    let schema_path = output.join("schema.capnp");
    fs::write(&schema_path, schema)?;
    
    // Compile the Cap'n Proto schema to Rust
    capnpc::CompilerCommand::new()
        .file(&schema_path)
        .output_path(&output)
        .src_prefix(&output)
        .run()
        .context("Failed to compile Cap'n Proto schema")?;

    // Modify the generated Cap'n Proto code to include Serde support
    let capnp_path = output.join("schema_capnp.rs");
    let mut capnp_code = fs::read_to_string(&capnp_path)
        .context("Failed to read generated Cap'n Proto code")?;

    // Add Serde imports and derives at the top
    let serde_imports = "#[cfg(feature = \"serde\")]\nuse serde::{Serialize, Deserialize};\n\n";
    capnp_code = serde_imports.to_string() + &capnp_code;

    // Add Serde derives to structs and enums
    for s in &structs {
        if s.has_serde {
            let derive = format!("#[cfg_attr(feature = \"serde\", derive(Serialize, Deserialize))]\n");
            capnp_code = capnp_code.replace(&format!("pub struct {}", s.name), &format!("{}\npub struct {}", derive, s.name));
        }
    }

    for e in &enums {
        if e.has_serde {
            let derive = format!("#[cfg_attr(feature = \"serde\", derive(Serialize, Deserialize))]\n");
            if e.variants.iter().any(|(_, ty)| ty.is_some()) {
                capnp_code = capnp_code.replace(&format!("pub struct {}", e.name), &format!("{}\npub struct {}", derive, e.name));
            } else {
                capnp_code = capnp_code.replace(&format!("pub enum {}", e.name), &format!("{}\npub enum {}", derive, e.name));
            }
        }
    }

    // Write the modified code back
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