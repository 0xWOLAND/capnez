use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Item, ItemStruct, ItemEnum, Ident, Generics, Attribute, Meta};

#[proc_macro_attribute]
pub fn capnp_bytes(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item);
    
    match input {
        Item::Struct(item) => {
            let mut attrs = item.attrs.clone();
            attrs.push(syn::parse_quote!(#[capnp_bytes]));
            let mut new_item = item.clone();
            new_item.attrs = attrs;
            impl_capnp_item(new_item)
        }
        _ => panic!("The #[capnp_bytes] attribute can only be used on structs"),
    }
}

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

fn has_capnp_bytes_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if let Meta::Path(path) = &attr.meta {
            path.segments.last().map_or(false, |seg| seg.ident == "capnp_bytes")
        } else {
            false
        }
    })
}

fn impl_capnp_item<T: quote::ToTokens + HasIdent + HasGenerics + HasAttrs>(item: T) -> TokenStream {
    let name = &item.ident();
    let (impl_generics, ty_generics, where_clause) = item.generics().split_for_impl();
    let is_bytes = has_capnp_bytes_attr(&item.attrs());
    
    TokenStream::from(quote! {
        #item

        impl #impl_generics #name #ty_generics #where_clause {
            pub fn capnp_schema() -> &'static str {
                include_str!(concat!(env!("OUT_DIR"), "/generated/schema.capnp"))
            }

            pub fn is_capnp_bytes() -> bool {
                #is_bytes
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

trait HasAttrs {
    fn attrs(&self) -> &[Attribute];
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

impl HasAttrs for ItemStruct {
    fn attrs(&self) -> &[Attribute] {
        &self.attrs
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

impl HasAttrs for ItemEnum {
    fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }
}
