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

trait SchemaComponent {
    fn push(&self);
}

trait SchemaWriter {
    fn write(&self, out: &mut String);
}

impl SchemaComponent for CapnpStruct {
    fn push(&self) {
        SCHEMA.lock().unwrap().0.push(self.clone());
    }
}

impl SchemaComponent for CapnpEnum {
    fn push(&self) {
        SCHEMA.lock().unwrap().1.push(self.clone());
    }
}

impl SchemaWriter for CapnpEnum {
    fn write(&self, out: &mut String) {
        if self.variants.iter().any(|v| v.ty.is_some()) {
            out.push_str(&format!("struct {} {{\n", self.name));
            for (i, v) in self.variants.iter().enumerate() {
                let ty = v.ty.as_ref().map_or("Void".to_string(), |t| t.to_string());
                out.push_str(&format!("  {} @{} :{};\n", v.name, i, ty));
            }
            out.push_str("}\n\n");
        } else {
            out.push_str(&format!("enum {} {{\n", self.name));
            for (i, v) in self.variants.iter().enumerate() {
                out.push_str(&format!("  {} @{};\n", v.name, i));
            }
            out.push_str("}\n\n");
        }
    }
}

impl SchemaWriter for CapnpStruct {
    fn write(&self, out: &mut String) {
        out.push_str(&format!("struct {} {{\n", self.name));
        for f in &self.fields {
            out.push_str(&format!("  {} @{} :{};\n", f.name, f.id, f.ty));
        }
        out.push_str("}\n\n");
    }
}

#[proc_macro_derive(CapnpDerive)]
pub fn capnp_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match &input.data {
        Data::Struct(_) => {
            let s = mk_struct(&input);
            s.push();
        }
        Data::Enum(data) => {
            let e = mk_enum(&input, data);
            e.push();
        }
        _ => panic!("CapnpDerive only supports structs and enums"),
    }
    write();
    TokenStream::new()
}

fn mk_struct(input: &DeriveInput) -> CapnpStruct {
    let name = input.ident.to_string();
    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => n.named.iter()
                .enumerate()
                .map(|(i, f)| Field {
                    name: f.ident.as_ref().unwrap().to_string(),
                    id: i,
                    ty: map_ty(&f.ty),
                })
                .collect(),
            _ => panic!("CapnpDerive only supports named structs"),
        },
        _ => panic!("CapnpDerive only supports structs"),
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
            Variant {
                name: v.ident.to_string(),
                ty,
            }
        })
        .collect();
    CapnpEnum { name, variants }
}

fn write() {
    let (structs, enums) = SCHEMA.lock().unwrap().clone();
    let ordered = sort_deps(&structs);
    
    let mut out = String::from("@0xabcdefabcdefabcdef;\n\n");
    
    // Write enums first
    for e in enums {
        e.write(&mut out);
    }
    
    // Then write structs
    for s in ordered {
        s.write(&mut out);
    }

    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("target/capnp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("generated.capnp"), out).unwrap();
}

fn map_ty(ty: &Type) -> CapnpType {
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let id = p.path.segments.last().unwrap().ident.to_string();
            match id.as_str() {
                "String" => CapnpType::Primitive(PrimitiveType::Text),
                "u32" => CapnpType::Primitive(PrimitiveType::UInt32),
                "u64" => CapnpType::Primitive(PrimitiveType::UInt64),
                "bool" => CapnpType::Primitive(PrimitiveType::Bool),
                "Option" => CapnpType::Optional(Box::new(extract_opt_ty(p))),
                "Vec" => CapnpType::List(Box::new(extract_list_ty(p))),
                name => if is_enum(name) {
                    CapnpType::Enum(name.to_string())
                } else {
                    CapnpType::Struct(name.to_string())
                }
            }
        }
        Type::Array(a) => CapnpType::List(Box::new(map_ty(&a.elem))),
        _ => panic!("Unsupported type"),
    }
}

fn is_enum(name: &str) -> bool {
    SCHEMA.lock().unwrap().1.iter().any(|e| e.name == name)
}

fn extract_opt_ty(p: &syn::TypePath) -> CapnpType {
    match &p.path.segments[0].arguments {
        PathArguments::AngleBracketed(args) => {
            if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                map_ty(inner_ty)
            } else {
                panic!("Option must have a type parameter")
            }
        }
        _ => panic!("Option must have angle bracketed arguments")
    }
}

fn extract_list_ty(p: &syn::TypePath) -> CapnpType {
    match &p.path.segments[0].arguments {
        PathArguments::AngleBracketed(args) => {
            if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                map_ty(inner_ty)
            } else {
                panic!("Vec must have a type parameter")
            }
        }
        _ => panic!("Vec must have angle bracketed arguments")
    }
}

fn sort_deps<'a>(items: &'a [CapnpStruct]) -> Vec<&'a CapnpStruct> {
    let mut seen = HashSet::new();
    let mut order = Vec::new();
    
    fn visit<'b>(s: &'b CapnpStruct, items: &'b [CapnpStruct], seen: &mut HashSet<String>, order: &mut Vec<&'b CapnpStruct>) {
        if !seen.insert(s.name.clone()) { return; }
        
        for f in &s.fields {
            if let Some(name) = get_struct_name(&f.ty) {
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
