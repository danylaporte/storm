use proc_macro2::TokenStream;
use syn::{spanned::Spanned, Data, DeriveInput, Error, Fields};

pub trait DeriveInputExt {
    fn input(&self) -> &DeriveInput;

    fn fields(&self) -> Result<&Fields, TokenStream> {
        let input = self.input();
        match &input.data {
            Data::Struct(s) => Ok(&s.fields),
            _ => Err(Error::new(input.span(), "Only struct are supported.").to_compile_error()),
        }
    }
}

impl DeriveInputExt for DeriveInput {
    fn input(&self) -> &DeriveInput {
        self
    }
}
