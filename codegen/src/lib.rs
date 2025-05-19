use anyhow::{Context, Result};
use std::{fs, path::PathBuf, env, collections::HashMap};
use walkdir::WalkDir;
use syn::{parse_file, Item, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument, Attribute, ItemTrait};

#[derive(Clone)]
enum CapnpType {
    Text,
    UInt32,
    UInt64,
    Bool,
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
            Self::Bool => write!(f, "Bool"),
            Self::List(inner) => write!(f, "List({})", inner),
            Self::Optional(inner) => write!(f, "union {{\n  value @0 :{};\n  none @1 :Void;\n}}", inner),
            Self::Struct(name) => write!(f, "{}", name),
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
struct CapnpInterface {
    name: String,
    methods: Vec<(String, Vec<(String, CapnpType)>, Option<CapnpType>)>,
}

fn has_serialization_attr(attrs: &[Attribute]) -> (bool, bool) {
    attrs.iter().fold((false, false), |(capnp, serde), attr| {
        if let Some(seg) = attr.path().segments.last() {
            match seg.ident.to_string().as_str() {
                "capnp" => (true, serde),
                "serde" => (capnp, true),
                "derive" => {
                    if let syn::Meta::List(list) = &attr.meta {
                        let has_serde = list.parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
                            .unwrap_or_default()
                            .iter()
                            .any(|meta| matches!(meta, syn::Meta::Path(p) if p.segments.last().map_or(false, |s| s.ident == "Serialize" || s.ident == "Deserialize")));
                        (capnp, serde || has_serde)
                    } else {
                        (capnp, serde)
                    }
                }
                _ => (capnp, serde),
            }
        } else {
            (capnp, serde)
        }
    })
}

fn map_ty(ty: &Type) -> CapnpType {
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let id = p.path.segments.last().unwrap().ident.to_string();
            match id.as_str() {
                "String" => CapnpType::Text,
                "u32" => CapnpType::UInt32,
                "u64" => CapnpType::UInt64,
                "bool" => CapnpType::Bool,
                "Option" => CapnpType::Optional(Box::new(extract_generic_ty(p))),
                "Vec" => CapnpType::List(Box::new(extract_generic_ty(p))),
                name => CapnpType::Struct(to_pascal_case(name)),
            }
        }
        Type::Array(a) => CapnpType::List(Box::new(map_ty(&a.elem))),
        _ => panic!("Unsupported type"),
    }
}

fn extract_generic_ty(p: &syn::TypePath) -> CapnpType {
    match &p.path.segments[0].arguments {
        PathArguments::AngleBracketed(args) => args.args.first()
            .and_then(|arg| match arg {
                GenericArgument::Type(inner_ty) => Some(map_ty(inner_ty)),
                _ => None
            })
            .unwrap_or_else(|| panic!("Generic type must have a type parameter")),
        _ => panic!("Generic type must have angle bracketed arguments"),
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
        })
        .collect()
}

fn to_camel_case(s: &str) -> String {
    let mut words = s.split('_');
    words.next().map_or(String::new(), |word| {
        let mut chars = word.chars();
        chars.next().map_or(String::new(), |c| c.to_lowercase().chain(chars).collect())
    }) + &words
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
        })
        .collect::<String>()
}

fn mk_struct(input: &DeriveInput, has_serde: bool) -> CapnpStruct {
    let name = to_pascal_case(&input.ident.to_string());
    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => n.named.iter()
                .enumerate()
                .map(|(i, f)| {
                    let field_name = f.ident.as_ref().unwrap().to_string();
                    (to_camel_case(&field_name), i, map_ty(&f.ty))
                })
                .collect(),
            _ => panic!("Only named structs are supported"),
        },
        _ => panic!("Only structs are supported"),
    };
    CapnpStruct { name, fields, has_serde }
}

fn mk_interface(input: &ItemTrait) -> CapnpInterface {
    let name = to_pascal_case(&input.ident.to_string());
    let methods = input.items.iter()
        .filter_map(|item| {
            if let syn::TraitItem::Fn(method) = item {
                let name = to_camel_case(&method.sig.ident.to_string());
                let params = method.sig.inputs.iter()
                    .filter_map(|arg| {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                                Some((to_camel_case(&pat_ident.ident.to_string()), map_ty(&pat_type.ty)))
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

pub fn generate_schema() -> Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let output = out_dir.join("generated");
    fs::create_dir_all(&output)?;
    
    let mut structs = Vec::new();
    let mut interfaces = Vec::new();
    
    // Walk through all .rs files
    for entry in WalkDir::new(manifest_dir.join("src"))
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
                        structs.push(mk_struct(&input, has_serde));
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

    // Generate schema
    let mut schema = String::from("@0xabcdefabcdefabcdef;\n\n");
    
    // Write structs and interfaces
    for s in &structs {
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
            if let Some(ret) = ret {
                schema.push_str(&format!(" -> {}", ret));
            }
            schema.push_str(";\n");
        }
        schema.push_str("}\n\n");
    }
    
    // Write and compile schema
    let schema_path = output.join("schema.capnp");
    fs::write(&schema_path, schema)?;
    
    capnpc::CompilerCommand::new()
        .file(&schema_path)
        .output_path(&output)
        .src_prefix(&output)
        .run()
        .context("Failed to compile Cap'n Proto schema")?;

    // Add Serde support
    let capnp_path = output.join("schema_capnp.rs");
    let mut capnp_code = fs::read_to_string(&capnp_path)
        .context("Failed to read generated Cap'n Proto code")?;

    let serde_imports = "#[cfg(feature = \"serde\")]\nuse serde::{Serialize, Deserialize};\n\n";
    capnp_code = serde_imports.to_string() + &capnp_code;

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