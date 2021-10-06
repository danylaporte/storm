use darling::FromMeta;

#[cfg(feature = "mssql")]
use inflector::Inflector;

#[cfg(feature = "mssql")]
use proc_macro2::TokenStream;

#[cfg(feature = "mssql")]
use syn::{spanned::Spanned, Field};

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, Eq, FromMeta, PartialEq)]
pub(crate) enum RenameAll {
    #[darling(rename = "camelCase")]
    CamelCase,

    #[darling(rename = "PascalCase")]
    PascalCase,

    #[darling(rename = "snake_case")]
    SnakeCase,
}

impl RenameAll {
    #[cfg(feature = "mssql")]
    pub fn column(
        this: Option<Self>,
        column: &Option<String>,
        field: &Field,
    ) -> Result<String, TokenStream> {
        if let Some(c) = column.as_ref().filter(|c| !c.is_empty()) {
            return Ok(c.clone());
        }

        let s = match field.ident.as_ref() {
            Some(v) => v.to_string(),
            None => {
                return Err(
                    syn::Error::new(field.span(), "Only struct are supported").to_compile_error()
                )
            }
        };

        Ok(match this {
            Some(r) => r.rename(s),
            None => s,
        })
    }

    #[cfg(feature = "mssql")]
    fn rename(&self, s: String) -> String {
        match self {
            Self::CamelCase => s.to_camel_case(),
            Self::PascalCase => s.to_pascal_case(),
            Self::SnakeCase => s.to_snake_case(),
        }
    }
}
