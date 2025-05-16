use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type};
use once_cell::sync::Lazy;
use std::{sync::Mutex, fs, env, path::Path};
use std::fmt;
use std::collections::{HashMap, HashSet};

static SCHEMA: Lazy<Mutex<Vec<CapnpStruct>>> = Lazy::new(|| Mutex::new(Vec::new()));

#[derive(Clone)]
struct CapnpStruct {
    name: String,
    fields: Vec<(String, usize, CapnpType)>,
}

#[derive(Clone)]
enum CapnpType {
    Text,
    UInt32,
    UInt64,
    Bool,
    Struct(String),
    List(Box<CapnpType>),
}

impl fmt::Display for CapnpType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CapnpType::Text       => write!(f, "Text"),
            CapnpType::UInt32     => write!(f, "UInt32"),
            CapnpType::UInt64     => write!(f, "UInt64"),
            CapnpType::Bool       => write!(f, "Bool"),
            CapnpType::Struct(n)  => write!(f, "{}", n),
            CapnpType::List(inner)=> write!(f, "List({})", inner),
        }
    }
}

#[proc_macro_derive(CapnpExport)]
pub fn capnp_export(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    add_struct(&input);
    write_schema();
    TokenStream::new()
}

fn add_struct(input: &DeriveInput) {
    let name = input.ident.to_string();
    let named = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => &n.named,
            _ => panic!("CapnpExport only supports named structs"),
        },
        _ => panic!("CapnpExport only supports structs"),
    };
    let fields = named.iter()
        .enumerate()
        .map(|(i, f)| (f.ident.as_ref().unwrap().to_string(), i, map_type(&f.ty)))
        .collect();
    SCHEMA.lock().unwrap_or_else(|e| e.into_inner()).push(CapnpStruct { name, fields });
}

fn write_schema() {
    let items = SCHEMA.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let ordered = topo_sort(&items);
    let mut out = String::from("@0xabcdefabcdefabcdef;\n\n");
    for s in ordered {
        out.push_str(&format!("struct {} {{\n", s.name));
        for (n, id, ty) in &s.fields {
            out.push_str(&format!("  {} @{} :{};\n", n, id, ty));
        }
        out.push_str("}\n\n");
    }
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let dir = Path::new(&manifest).join("target/capnp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("generated.capnp"), out).unwrap();
}

fn map_type(ty: &Type) -> CapnpType {
    match ty {
        Type::Path(p) if p.qself.is_none() => {
            let id = p.path.segments.last().unwrap().ident.to_string();
            match id.as_str() {
                "String" => CapnpType::Text,
                "u32"    => CapnpType::UInt32,
                "u64"    => CapnpType::UInt64,
                "bool"   => CapnpType::Bool,
                other     => CapnpType::Struct(other.into()),
            }
        }
        Type::Array(a) => CapnpType::List(Box::new(map_type(&a.elem))),
        _ => panic!("Unsupported type"),
    }
}

fn topo_sort<'a>(items: &'a [CapnpStruct]) -> Vec<&'a CapnpStruct> {
    use std::collections::HashSet;
    let mut visited = HashSet::new();
    let mut order = Vec::new();
    fn dfs<'b>(s: &'b CapnpStruct, items: &'b [CapnpStruct], visited: &mut HashSet<&'b str>, order: &mut Vec<&'b CapnpStruct>) {
        if !visited.insert(s.name.as_str()) { return; }
        for (_, _, ty) in &s.fields {
            if let CapnpType::Struct(ref name) = ty {
                if let Some(dep) = items.iter().find(|x| x.name == *name) {
                    dfs(dep, items, visited, order);
                }
            } else if let CapnpType::List(inner) = ty {
                let mut inner_ty = inner.as_ref();
                // recurse into nested lists
                while let CapnpType::List(next) = inner_ty {
                    inner_ty = next.as_ref();
                }
                if let CapnpType::Struct(ref name) = inner_ty {
                    if let Some(dep) = items.iter().find(|x| x.name == *name) {
                        dfs(dep, items, visited, order);
                    }
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
