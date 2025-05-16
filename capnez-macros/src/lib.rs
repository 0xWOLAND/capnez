use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct, ItemEnum, ItemTrait};

#[proc_macro_attribute]
pub fn capnp(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item);
    
    match input {
        syn::Item::Struct(item) => impl_capnp_struct(item),
        syn::Item::Enum(item) => impl_capnp_enum(item),
        syn::Item::Trait(item) => impl_capnp_trait(item),
        _ => panic!("The #[capnp] attribute can only be used on structs, enums, and traits"),
    }
}

fn impl_capnp_struct(item: ItemStruct) -> TokenStream {
    let name = &item.ident;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    
    let expanded = quote! {
        #item

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn capnp_schema() -> &'static str {
                include_str!(concat!(env!("OUT_DIR"), "/generated/schema.capnp"))
            }
        }
    };
    
    TokenStream::from(expanded)
}

fn impl_capnp_enum(item: ItemEnum) -> TokenStream {
    let name = &item.ident;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    
    let expanded = quote! {
        #item

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn capnp_schema() -> &'static str {
                include_str!(concat!(env!("OUT_DIR"), "/generated/schema.capnp"))
            }
        }
    };
    
    TokenStream::from(expanded)
}

fn impl_capnp_trait(item: ItemTrait) -> TokenStream {
    TokenStream::from(quote! {
        #item
    })
}