//use crate::AttrsExt;
use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{spanned::Spanned, Error, Field, Ident};

pub trait FieldExt {
    fn field(&self) -> &Field;

    fn ident(&self) -> Result<&Ident, TokenStream> {
        let f = self.field();
        f.ident
            .as_ref()
            .ok_or_else(|| Error::new(f.span(), "Ident expected.").to_compile_error())
    }

    fn type_info(&self) -> TypeInfo {
        let s = self.field().ty.to_token_stream().to_string();

        if s.find("AsyncOnceCell").is_some() {
            return TypeInfo::AsyncOnceCell;
        }

        TypeInfo::Other
    }
}

impl FieldExt for Field {
    fn field(&self) -> &Field {
        self
    }
}

pub enum TypeInfo {
    AsyncOnceCell,
    Other,
}
