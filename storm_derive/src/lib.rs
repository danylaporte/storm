extern crate proc_macro;

#[macro_use]
mod macros;

mod ctx;
mod derive_input_ext;
mod field_ext;

use derive_input_ext::DeriveInputExt;
use field_ext::FieldExt;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Ctx, attributes(column, key, table, translated))]
pub fn ctx(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ctx::generate(&input).into()
}
