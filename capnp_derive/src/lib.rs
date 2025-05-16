use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument, ItemTrait};
use once_cell::sync::Lazy;
use std::{sync::Mutex, fs, env, path::Path};
use std::fmt;
use std::collections::HashSet;

type Schema = (Vec<CapnpStruct>, Vec<CapnpEnum>, Vec<CapnpInterface>);
static SCHEMA: Lazy<Mutex<Schema>> = Lazy::new(|| Mutex::new((Vec::new(), Vec::new(), Vec::new())));

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

#[derive(Clone)]
struct CapnpInterface {
    name: String,
    methods: Vec<Method>,
}

#[derive(Clone)]
struct Method {
    name: String,
    id: usize,
    params: Vec<Param>,
    ret: Option<CapnpType>,
}

#[derive(Clone)]
struct Param {
    name: String,
    ty: CapnpType,
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

trait SchemaPush {
    fn push(&self);
}

trait SchemaWriter {
    fn write(&self, out: &mut String);
}

impl SchemaPush for CapnpStruct {
    fn push(&self) {
        SCHEMA.lock().unwrap().0.push(self.clone());
    }
}

impl SchemaPush for CapnpEnum {
    fn push(&self) {
        SCHEMA.lock().unwrap().1.push(self.clone());
    }
}

impl SchemaPush for CapnpInterface {
    fn push(&self) {
        SCHEMA.lock().unwrap().2.push(self.clone());
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

impl SchemaWriter for CapnpInterface {
    fn write(&self, out: &mut String) {
        out.push_str(&format!("interface {} {{\n", self.name));
        for m in &self.methods {
            out.push_str(&format!("  {} @{} (", m.name, m.id));
            for (i, p) in m.params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("{} :{}", p.name, p.ty));
            }
            out.push_str(")");
            if let Some(ret) = &m.ret {
                out.push_str(&format!(" -> {}", ret));
            }
            out.push_str(";\n");
        }
        out.push_str("}\n\n");
    }
}

#[proc_macro_attribute]
pub fn capnp(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::Item);
    match input {
        syn::Item::Struct(s) => {
            let s = mk_struct(&DeriveInput {
                attrs: s.attrs,
                vis: s.vis,
                ident: s.ident,
                generics: s.generics,
                data: syn::Data::Struct(syn::DataStruct {
                    struct_token: s.struct_token,
                    fields: s.fields,
                    semi_token: s.semi_token,
                }),
            });
            s.push();
        }
        syn::Item::Enum(e) => {
            let variants = e.variants.clone();
            let e = mk_enum(&DeriveInput {
                attrs: e.attrs,
                vis: e.vis,
                ident: e.ident,
                generics: e.generics,
                data: syn::Data::Enum(syn::DataEnum {
                    enum_token: e.enum_token,
                    brace_token: e.brace_token,
                    variants: e.variants,
                }),
            }, &syn::DataEnum {
                enum_token: e.enum_token,
                brace_token: e.brace_token,
                variants,
            });
            e.push();
        }
        syn::Item::Trait(t) => {
            let iface = mk_interface(&t);
            iface.push();
        }
        _ => panic!("capnp attribute only supports structs, enums, and traits"),
    }
    write();
    TokenStream::new()
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
                                let name = pat_ident.ident.to_string();
                                let ty = map_ty(&pat_type.ty);
                                Some(Param { name, ty })
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
                Some(Method {
                    name,
                    id: 0, // TODO: Generate unique IDs
                    params,
                    ret,
                })
            } else {
                None
            }
        })
        .collect();
    CapnpInterface { name, methods }
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
    let (structs, enums, interfaces) = SCHEMA.lock().unwrap().clone();
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

    // Finally write interfaces
    for i in interfaces {
        i.write(&mut out);
    }

    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("target/capnp");
    fs::create_dir_all(&dir).unwrap();
    let schema_path = dir.join("generated.capnp");
    fs::write(&schema_path, out).unwrap();

    // Run the Cap'n Proto compiler
    capnpc::CompilerCommand::new()
        .file(schema_path.clone())
        .run()
        .expect("compiling schema");

    println!("Built capnp schema to {}", schema_path.display());
    println!("cargo:rustc-cfg=include_capnp");
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
