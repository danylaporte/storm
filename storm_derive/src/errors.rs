use proc_macro2::TokenStream;
use quote::quote;

pub(crate) trait Errors {
    fn result(&self) -> Result<(), TokenStream>;
}

impl Errors for Vec<TokenStream> {
    fn result(&self) -> Result<(), TokenStream> {
        if self.is_empty() {
            Ok(())
        } else {
            let v = self;
            Err(quote! { #(#v)* })
        }
    }
}
