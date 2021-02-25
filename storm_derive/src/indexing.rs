use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Error, FnArg, Item, Pat};

#[allow(unused_variables)]
pub(crate) fn indexing(item: Item) -> TokenStream {
    let f = match &item {
        Item::Fn(f) => f,
        _ => return Error::new(item.span(), "Only function is supported.").to_compile_error(),
    };

    //let mut wheres = Vec::new();

    for arg in &f.sig.inputs {
        let t = match arg {
            FnArg::Typed(t) => t,
            _ => return Error::new(arg.span(), "self argument is supported.").to_compile_error(),
        };

        let ident = match &*t.pat {
            Pat::Ident(ident) => ident,
            _ => return Error::new(t.pat.span(), "self argument is supported.").to_compile_error(),
        };

        // wheres.push(quote! {
        //     #t: storm::Init<
        // })
    }

    quote!(#item)

    // quote! {
    //     #[async_trait::async_trait]
    //     impl<F, P, T> storm::Init<P> for Index<T, F>
    //     where
    //         #wheres
    //     {
    //         async fn init(provider: &P) -> Result<Self> {
    //             todo!()
    //         }
    //     }
    // }
}
