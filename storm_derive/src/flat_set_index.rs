use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Error, Ident, Item, ItemFn};

pub(crate) fn flat_set_index(item: Item) -> TokenStream {
    match &item {
        Item::Fn(f) => indexing_fn(f),
        _ => Error::new(item.span(), "Only function is supported.").to_compile_error(),
    }
}

fn indexing_fn(f: &ItemFn) -> TokenStream {
    let snake = f.sig.ident.to_string();
    let name = snake.to_pascal_case();
    let adapt = Ident::new(&format!("{name}Adapt"), f.sig.ident.span());
    let alias = Ident::new(&name, f.sig.ident.span());
    let init = Ident::new(&format!("__{snake}_init"), f.sig.ident.span());

    quote! {
        storm::flat_set_adapt! {
            #adapt,
            #alias,
            #init,
            #f
        }
    }
}
