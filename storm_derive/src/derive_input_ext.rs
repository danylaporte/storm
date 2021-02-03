use proc_macro2::TokenStream;
use syn::{spanned::Spanned, Data, DeriveInput, Error, Field, Fields};

pub trait DeriveInputExt {
    fn input(&self) -> &DeriveInput;

    fn fields(&self) -> Result<&Fields, TokenStream> {
        let input = self.input();
        match &input.data {
            Data::Struct(s) => Ok(&s.fields),
            _ => Err(Error::new(input.span(), "Only struct are supported.").to_compile_error()),
        }
    }

    fn tuple_single_field(&self) -> Result<&Field, TokenStream> {
        let fields = self.fields()?;

        let mut iter = fields.into_iter();

        match (iter.next(), iter.next()) {
            (Some(f), None) if f.ident.is_none() => Ok(f),
            _ => Err(Error::new(
                self.input().span(),
                "Only tuple with one field are supported.",
            )
            .to_compile_error()),
        }
    }
}

impl DeriveInputExt for DeriveInput {
    fn input(&self) -> &DeriveInput {
        self
    }
}
