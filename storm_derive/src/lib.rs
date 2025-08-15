#![allow(clippy::indexing_slicing)]

extern crate proc_macro;

#[macro_use]
mod macros;

mod ctx;
mod derive_input_ext;
#[cfg(feature = "mssql")]
mod errors;
mod field_ext;
mod flat_set_index;
mod hierarchy;
mod indexing;
mod locks_await;
#[cfg(feature = "mssql")]
mod mssql;
mod node_set_index;
mod noop;
mod register;
mod rename_all;
mod single_set;
#[cfg(feature = "mssql")]
mod string_ext;
mod token_stream_ext;
mod tree_index;
mod type_ext;

use derive_input_ext::DeriveInputExt;
#[cfg(feature = "mssql")]
use errors::Errors;
#[cfg(feature = "mssql")]
use field_ext::FieldExt;
use proc_macro::TokenStream;
#[cfg(feature = "mssql")]
use rename_all::RenameAll;
#[cfg(feature = "mssql")]
use string_ext::StringExt;
use syn::{parse_macro_input, DeriveInput, Item};
use type_ext::TypeExt;

#[proc_macro_derive(Ctx, attributes(storm))]
pub fn ctx(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ctx::generate(&input).into()
}

#[proc_macro_attribute]
pub fn flat_set_index(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);
    flat_set_index::flat_set_index(item).into()
}

#[proc_macro_attribute]
pub fn hierarchy(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);
    hierarchy::hierarchy(item).into()
}

#[proc_macro_attribute]
pub fn indexing(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);
    indexing::indexing(item).into()
}

#[proc_macro_derive(LocksAwait, attributes(storm))]
pub fn locks_await(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    locks_await::locks_await(&input).into()
}

#[cfg(feature = "mssql")]
#[proc_macro_derive(MssqlDelete, attributes(storm))]
pub fn mssql_delete(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    mssql::delete(&input).into()
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

#[proc_macro_attribute]
pub fn node_set_index(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);
    node_set_index::node_set_index(item).into()
}

#[proc_macro_derive(NoopDelete)]
pub fn noop_delete(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    noop::delete(&input).into()
}

#[proc_macro_derive(NoopLoad)]
pub fn noop_load(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    noop::load(&input).into()
}

#[proc_macro_derive(NoopSave)]
pub fn noop_save(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    noop::save(&input).into()
}

#[proc_macro_attribute]
pub fn register(attr: TokenStream, item: TokenStream) -> TokenStream {
    register::register(attr, item)
}

#[proc_macro_attribute]
pub fn single_set(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);
    single_set::single_set(item).into()
}

#[proc_macro_attribute]
pub fn tree_index(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as Item);
    tree_index::tree_index(item).into()
}
