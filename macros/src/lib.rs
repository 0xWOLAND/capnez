use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Item, ItemStruct, ItemEnum, Ident, Generics};

#[proc_macro_attribute]
pub fn capnp(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item);
    
    match input {
        Item::Struct(item) => impl_capnp_item(item),
        Item::Enum(item) => impl_capnp_item(item),
        Item::Trait(item) => TokenStream::from(quote! { #item }),
        _ => panic!("The #[capnp] attribute can only be used on structs, enums, and traits"),
    }
}

fn impl_capnp_item<T: quote::ToTokens + HasIdent + HasGenerics>(item: T) -> TokenStream {
    let name = &item.ident();
    let (impl_generics, ty_generics, where_clause) = item.generics().split_for_impl();
    
    TokenStream::from(quote! {
        #item

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn capnp_schema() -> &'static str {
                include_str!(concat!(env!("OUT_DIR"), "/generated/schema.capnp"))
            }
        }
    })
}

trait HasIdent {
    fn ident(&self) -> &Ident;
}

trait HasGenerics {
    fn generics(&self) -> &Generics;
}

impl HasIdent for ItemStruct {
    fn ident(&self) -> &Ident {
        &self.ident
    }
}

impl HasGenerics for ItemStruct {
    fn generics(&self) -> &Generics {
        &self.generics
    }
}

impl HasIdent for ItemEnum {
    fn ident(&self) -> &Ident {
        &self.ident
    }
}

impl HasGenerics for ItemEnum {
    fn generics(&self) -> &Generics {
        &self.generics
    }
}
