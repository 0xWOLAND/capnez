use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument};
use once_cell::sync::Lazy;
use std::{sync::Mutex, fs, env, path::Path};
use std::fmt;
use std::collections::HashSet;

type Schema = (Vec<CapnpStruct>, Vec<CapnpEnum>);
static SCHEMA: Lazy<Mutex<Schema>> = Lazy::new(|| Mutex::new((Vec::new(), Vec::new())));

#[derive(Clone)]
struct CapnpStruct {
    name: String,
    fields: Vec<Field>,
}

#[derive(Clone)]
struct Field {
    name: String,
    id: usize,
    ty: CapnpType,
}

#[derive(Clone)]
enum CapnpType {
    Primitive(PrimitiveType),
    Struct(String),
    List(Box<CapnpType>),
    Enum(String),
    Optional(Box<CapnpType>),
}

#[derive(Clone)]
enum PrimitiveType {
    Text,
    UInt32,
    UInt64,
    Bool,
}

impl fmt::Display for CapnpType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CapnpType::Primitive(p) => write!(f, "{}", p),
            CapnpType::Struct(n) => write!(f, "{}", n),
            CapnpType::List(inner) => write!(f, "List({})", inner),
            CapnpType::Enum(n) => write!(f, "{}", n),
            CapnpType::Optional(inner) => {
                write!(f, "union {{\n  value @0 :{};\n  none @1 :Void;\n}}", inner)
            }
        }
    }
}

impl fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimitiveType::Text => write!(f, "Text"),
            PrimitiveType::UInt32 => write!(f, "UInt32"),
            PrimitiveType::UInt64 => write!(f, "UInt64"),
            PrimitiveType::Bool => write!(f, "Bool"),
        }
    }
}

#[derive(Clone)]
struct CapnpEnum {
    name: String,
    variants: Vec<Variant>,
}

#[derive(Clone)]
struct Variant {
    name: String,
    ty: Option<CapnpType>,
}

#[proc_macro_derive(CapnpDerive)]
pub fn capnp_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match &input.data {
        Data::Struct(_) => add_struct(&input),
        Data::Enum(data) => add_enum(&input, data),
        _ => panic!("CapnpDerive only supports structs and enums"),
    }
    write_schema();
    TokenStream::new()
}

fn add_struct(input: &DeriveInput) {
    let name = input.ident.to_string();
    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => n.named.iter()
                .enumerate()
                .map(|(i, f)| Field {
                    name: f.ident.as_ref().unwrap().to_string(),
                    id: i,
                    ty: map_type(&f.ty),
                })
                .collect(),
            _ => panic!("CapnpDerive only supports named structs"),
        },
        _ => panic!("CapnpDerive only supports structs"),
    };
    SCHEMA.lock().unwrap().0.push(CapnpStruct { name, fields });
}

fn add_enum(input: &DeriveInput, data: &syn::DataEnum) {
    let name = input.ident.to_string();
    let variants = data.variants.iter()
        .map(|v| {
            let variant_type = match &v.fields {
                syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => 
                    Some(map_type(&fields.unnamed[0].ty)),
                syn::Fields::Unnamed(_) => panic!("Enum variants must have exactly one unnamed field"),
                _ => None,
            };
            Variant {
                name: v.ident.to_string(),
                ty: variant_type,
            }
        })
        .collect();
    SCHEMA.lock().unwrap().1.push(CapnpEnum { name, variants });
}

fn write_schema() {
    let (structs, enums) = SCHEMA.lock().unwrap().clone();
    let ordered = topo_sort(&structs);
    
    let mut out = String::from("@0xabcdefabcdefabcdef;\n\n");
    
    // Write enums first
    for e in enums {
        write_enum(&mut out, &e);
    }
    
    // Then write structs
    for s in ordered {
        write_struct(&mut out, s);
    }

    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("target/capnp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("generated.capnp"), out).unwrap();
}

fn write_enum(out: &mut String, e: &CapnpEnum) {
    if e.variants.iter().any(|v| v.ty.is_some()) {
        out.push_str(&format!("struct {} {{\n", e.name));
        for (i, v) in e.variants.iter().enumerate() {
            let type_str = v.ty.as_ref().map_or("Void".to_string(), |t| t.to_string());
            out.push_str(&format!("  {} @{} :{};\n", v.name, i, type_str));
        }
        out.push_str("}\n\n");
    } else {
        out.push_str(&format!("enum {} {{\n", e.name));
        for (i, v) in e.variants.iter().enumerate() {
            out.push_str(&format!("  {} @{};\n", v.name, i));
        }
        out.push_str("}\n\n");
    }
}

fn write_struct(out: &mut String, s: &CapnpStruct) {
    out.push_str(&format!("struct {} {{\n", s.name));
    for f in &s.fields {
        out.push_str(&format!("  {} @{} :{};\n", f.name, f.id, f.ty));
    }
    out.push_str("}\n\n");
}

fn map_type(ty: &Type) -> CapnpType {
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let id = p.path.segments.last().unwrap().ident.to_string();
            match id.as_str() {
                "String" => CapnpType::Primitive(PrimitiveType::Text),
                "u32" => CapnpType::Primitive(PrimitiveType::UInt32),
                "u64" => CapnpType::Primitive(PrimitiveType::UInt64),
                "bool" => CapnpType::Primitive(PrimitiveType::Bool),
                "Option" => CapnpType::Optional(Box::new(extract_option_type(p))),
                name => if is_enum(name) {
                    CapnpType::Enum(name.to_string())
                } else {
                    CapnpType::Struct(name.to_string())
                }
            }
        }
        Type::Array(a) => CapnpType::List(Box::new(map_type(&a.elem))),
        _ => panic!("Unsupported type"),
    }
}

fn is_enum(name: &str) -> bool {
    SCHEMA.lock().unwrap().1.iter().any(|e| e.name == name)
}

fn extract_option_type(p: &syn::TypePath) -> CapnpType {
    match &p.path.segments[0].arguments {
        PathArguments::AngleBracketed(args) => {
            if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                map_type(inner_ty)
            } else {
                panic!("Option must have a type parameter")
            }
        }
        _ => panic!("Option must have angle bracketed arguments")
    }
}

fn topo_sort<'a>(items: &'a [CapnpStruct]) -> Vec<&'a CapnpStruct> {
    let mut visited = HashSet::new();
    let mut order = Vec::new();
    
    fn dfs<'b>(s: &'b CapnpStruct, items: &'b [CapnpStruct], visited: &mut HashSet<String>, order: &mut Vec<&'b CapnpStruct>) {
        if !visited.insert(s.name.clone()) { return; }
        
        for f in &s.fields {
            if let Some(name) = extract_struct_name(&f.ty) {
                if let Some(dep) = items.iter().find(|x| x.name == name) {
                    dfs(dep, items, visited, order);
                }
            }
        }
        order.push(s);
    }
    
    for s in items {
        dfs(s, items, &mut visited, &mut order);
    }
    order
}

fn extract_struct_name(ty: &CapnpType) -> Option<String> {
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
