use proc_macro2::TokenStream;
use quote::quote;

pub(crate) trait TokenStreamExt {
    /// Transform into a tokenstream.
    fn ts(self) -> TokenStream;
}

impl<I> TokenStreamExt for I
where
    I: IntoIterator<Item = TokenStream>,
{
    fn ts(self) -> TokenStream {
        let v = self.into_iter();
        quote!(#(#v)*)
    }
}
