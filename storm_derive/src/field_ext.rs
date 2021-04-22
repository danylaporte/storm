//use crate::AttrsExt;
use proc_macro2::TokenStream;
use syn::{spanned::Spanned, Error, Field, Ident};

pub trait FieldExt {
    fn field(&self) -> &Field;

    fn ident(&self) -> Result<&Ident, TokenStream> {
        let f = self.field();
        f.ident
            .as_ref()
            .ok_or_else(|| Error::new(f.span(), "Ident expected.").to_compile_error())
    }
}

impl FieldExt for Field {
    fn field(&self) -> &Field {
        self
    }
}
