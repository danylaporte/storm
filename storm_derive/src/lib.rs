extern crate proc_macro;

#[macro_use]
mod macros;

mod attrs_ext;
mod ctx;
mod derive_input_ext;
#[cfg(any(feature = "postgres", feature = "mssql"))]
mod errors;
mod field_ext;
mod mssql;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(any(feature = "postgres", feature = "mssql"))]
mod string_ext;
mod token_stream_ext;

use attrs_ext::AttrsExt;
use derive_input_ext::DeriveInputExt;
#[cfg(any(feature = "postgres", feature = "mssql"))]
use errors::Errors;
use field_ext::FieldExt;
use proc_macro::TokenStream;
#[cfg(any(feature = "postgres", feature = "mssql"))]
use string_ext::StringExt;
use syn::{parse_macro_input, DeriveInput};
use token_stream_ext::TokenStreamExt;

#[proc_macro_derive(Ctx)]
pub fn ctx(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ctx::generate(&input).into()
}

#[cfg(feature = "postgres")]
#[proc_macro_derive(FromSql)]
pub fn from_sql(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    postgres::from_sql(&input).into()
}

#[cfg(feature = "postgres")]
#[proc_macro_derive(Load, attributes(column, key, table))]
pub fn load(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    postgres::load(&input).into()
}

#[cfg(feature = "postgres")]
#[proc_macro_derive(ToSql)]
pub fn to_sql(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    postgres::to_sql(&input).into()
}

#[cfg(feature = "postgres")]
#[proc_macro_derive(Upsert, attributes(column, key, table))]
pub fn upsert(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    postgres::upsert(&input).into()
}

#[cfg(feature = "mssql")]
#[proc_macro_derive(MssqlLoad, attributes(storm))]
pub fn mssql_load(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    mssql::load(&input).into()
}

#[cfg(feature = "mssql")]
#[proc_macro_derive(MssqlSave, attributes(storm))]
pub fn mssql_save(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    mssql::save(&input).into()
}
