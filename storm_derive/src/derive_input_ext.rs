use crate::AttrsExt;
use proc_macro2::TokenStream;
use syn::{spanned::Spanned, Data, DeriveInput, Error, Field, Fields, LitStr};

pub trait DeriveInputExt {
    fn input(&self) -> &DeriveInput;

    fn fields(&self) -> Result<&Fields, TokenStream> {
        let input = self.input();
        match &input.data {
            Data::Struct(s) => Ok(&s.fields),
            _ => Err(Error::new(input.span(), "Only struct are supported.").to_compile_error()),
        }
    }

    fn table(&self) -> Result<String, TokenStream> {
        self.table_lit().map(|l| l.value())
    }

    fn table_lit(&self) -> Result<LitStr, TokenStream> {
        let input = self.input();

        let table = input.attrs.parse_attr::<LitStr>("table")?.ok_or_else(|| {
            Error::new(input.span(), "table attribute expected").to_compile_error()
        })?;

        //check_table_format(&table).map_err(|e| e.to_compile_error())?;
        Ok(table)
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
