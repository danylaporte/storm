use proc_macro2::TokenStream;
use quote::quote;

pub(crate) trait TokenStreamExt {
    /// Transform into a tokenstream.
    fn ts(self) -> TokenStream;
}

impl TokenStreamExt for Vec<TokenStream> {
    fn ts(self) -> TokenStream {
        let v = self;
        quote!(#(#v)*)
    }
}
