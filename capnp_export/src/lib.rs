extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Type};
use std::{fs, path::Path};

#[proc_macro_derive(CapnpExport)]
pub fn capnp_export(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = input.ident.to_string();

    let fields = match input.data {
        syn::Data::Struct(syn::DataStruct { fields: syn::Fields::Named(f), .. }) => f.named,
        _ => panic!("CapnpExport only supports named structs"),
    };

    let schema = format!(
        "@0xabcdefabcdefabcdef;\n\nstruct {} {{\n{}}}\n",
        struct_name,
        fields.iter()
            .enumerate()
            .map(|(i, field)| {
                let name = field.ident.as_ref().unwrap();
                format!("  {} @{} :{};\n", name, i, rust_to_capnp_type(&field.ty))
            })
            .collect::<String>()
    );

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let target_dir = Path::new(&manifest_dir).join("target/capnp");
    fs::create_dir_all(&target_dir).expect("Failed to create capnp directory");
    fs::write(target_dir.join("generated.capnp"), schema).expect("Failed to write schema file");

    TokenStream::new()
}

fn rust_to_capnp_type(ty: &Type) -> &'static str {
    match ty {
        Type::Path(type_path) => {
            let ident = type_path.path.get_ident()
                .expect("unsupported complex type");
            match ident.to_string().as_str() {
                "String" => "Text",
                "u32" => "UInt32",
                "u64" => "UInt64",
                "bool" => "Bool",
                other => panic!("unsupported field type: {}", other),
            }
        }
        _ => panic!("unsupported type structure"),
    }
} 