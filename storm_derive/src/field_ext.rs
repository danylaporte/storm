use proc_macro2::TokenStream;
use syn::{spanned::Spanned, Error, Field, Ident, Type};

pub trait FieldExt {
    fn field(&self) -> &Field;

    fn ident(&self) -> Result<&Ident, TokenStream> {
        let f = self.field();
        f.ident
            .as_ref()
            .ok_or_else(|| Error::new(f.span(), "Ident expected.").to_compile_error())
    }

    fn ty(&self) -> &Type {
        &self.field().ty
    }
}

impl FieldExt for Field {
    fn field(&self) -> &Field {
        self
    }
}
