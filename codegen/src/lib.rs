use anyhow::{Context, Result};
use std::{fs, path::{Path, PathBuf}, collections::HashSet, env};
use walkdir::WalkDir;
use syn::{parse_file, Item, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument, ItemTrait};

#[derive(Clone)]
pub struct CapnpStruct {
    name: String,
    fields: Vec<(String, usize, CapnpType)>,
}

#[derive(Clone)]
pub struct CapnpEnum {
    name: String,
    variants: Vec<(String, Option<CapnpType>)>,
}

#[derive(Clone)]
pub struct CapnpInterface {
    name: String,
    methods: Vec<(String, Vec<(String, CapnpType)>, Option<CapnpType>)>,
}

#[derive(Clone)]
enum CapnpType {
    Primitive(&'static str),
    Struct(String),
    List(Box<CapnpType>),
    Enum(String),
    Optional(Box<CapnpType>),
}

impl std::fmt::Display for CapnpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapnpType::Primitive(p) => write!(f, "{}", p),
            CapnpType::Struct(n) => write!(f, "{}", n),
            CapnpType::List(inner) => write!(f, "List({})", inner),
            CapnpType::Enum(n) => write!(f, "{}", n),
            CapnpType::Optional(inner) => write!(f, "union {{\n  value @0 :{};\n  none @1 :Void;\n}}", inner),
        }
    }
}

fn map_ty(ty: &Type) -> CapnpType {
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let id = p.path.segments.last().unwrap().ident.to_string();
            match id.as_str() {
                "String" => CapnpType::Primitive("Text"),
                "u32" => CapnpType::Primitive("UInt32"),
                "u64" => CapnpType::Primitive("UInt64"),
                "bool" => CapnpType::Primitive("Bool"),
                "Option" => CapnpType::Optional(Box::new(extract_generic_ty(p))),
                "Vec" => CapnpType::List(Box::new(extract_generic_ty(p))),
                name => CapnpType::Struct(name.to_string())
            }
        }
        Type::Array(a) => CapnpType::List(Box::new(map_ty(&a.elem))),
        _ => panic!("Unsupported type"),
    }
}

fn extract_generic_ty(p: &syn::TypePath) -> CapnpType {
    match &p.path.segments[0].arguments {
        PathArguments::AngleBracketed(args) => {
            if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                map_ty(inner_ty)
            } else {
                panic!("Generic type must have a type parameter")
            }
        }
        _ => panic!("Generic type must have angle bracketed arguments")
    }
}

fn mk_struct(input: &DeriveInput) -> CapnpStruct {
    let name = input.ident.to_string();
    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => n.named.iter()
                .enumerate()
                .map(|(i, f)| (f.ident.as_ref().unwrap().to_string(), i, map_ty(&f.ty)))
                .collect(),
            _ => panic!("Only named structs are supported"),
        },
        _ => panic!("Only structs are supported"),
    };
    CapnpStruct { name, fields }
}

fn mk_enum(input: &DeriveInput, data: &syn::DataEnum) -> CapnpEnum {
    let name = input.ident.to_string();
    let variants = data.variants.iter()
        .map(|v| {
            let ty = match &v.fields {
                syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => 
                    Some(map_ty(&fields.unnamed[0].ty)),
                syn::Fields::Unnamed(_) => panic!("Enum variants must have exactly one unnamed field"),
                _ => None,
            };
            (v.ident.to_string(), ty)
        })
        .collect();
    CapnpEnum { name, variants }
}

fn mk_interface(input: &ItemTrait) -> CapnpInterface {
    let name = input.ident.to_string();
    let methods = input.items.iter()
        .filter_map(|item| {
            if let syn::TraitItem::Fn(method) = item {
                let name = method.sig.ident.to_string();
                let params = method.sig.inputs.iter()
                    .filter_map(|arg| {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                Some((pat_ident.ident.to_string(), map_ty(&pat_type.ty)))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect();
                let ret = match &method.sig.output {
                    syn::ReturnType::Type(_, ty) => Some(map_ty(&ty)),
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
            
        // Parse the Rust file using syn
        let file = parse_file(&content)
            .with_context(|| format!("Failed to parse {}", entry.path().display()))?;
            
        // Process each item in the file
        for item in file.items {
            match item {
                Item::Struct(s) => {
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
                    structs.push(mk_struct(&input));
                }
                Item::Enum(e) => {
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
                    }));
                }
                Item::Trait(t) => interfaces.push(mk_interface(&t)),
                _ => {}
            }
        }
    }
    
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
    
    // Create output directory if it doesn't exist
    fs::create_dir_all(&output)?;
    
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
        
    Ok(())
} 