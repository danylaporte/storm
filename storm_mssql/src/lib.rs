mod client_factory;
mod delete;
mod entity_diff;
mod execute;
mod field_diff;
mod filter_sql;
mod from_sql;
#[doc(hidden)]
pub mod load_sink;
#[doc(hidden)]
pub mod metrics_helper;
mod mssql_factory;
mod mssql_meta;
mod mssql_provider;
mod noop_identity;
mod parameter;
mod query_rows;
mod save_entity_part;
mod schema;
mod to_sql;
mod transaction_scoped;
mod upsert_builder;

pub use client_factory::ClientFactory;
pub use delete::_delete;
pub use entity_diff::*;
pub use execute::*;
pub use field_diff::*;
pub use filter_sql::*;
pub use from_sql::{_macro_load_field, FromSql};
// The sink traits stay in `load_sink`: they have blanket impls, so a glob import of this crate
// must not bring their methods into scope.
pub use load_sink::{_load_all, _load_one, _load_rows, _query_rows_each, MssqlLoadEntity};
pub use mssql_factory::MssqlFactory;
pub use mssql_meta::MssqlMeta;
pub use mssql_provider::{MssqlProvider, MssqlTransactionGuard};
pub use noop_identity::noop_identity;
pub use parameter::{Parameter, into_column_data_static};
pub use query_rows::QueryRows;
pub use save_entity_part::SaveEntityPart;
pub use schema::{
    ColumnSchema, EntitySchema, ForeignKeySchema, MssqlSchema, UniqueSchema, check_schema,
    test_entity,
};
pub use serde_json;
use storm::ProviderContainer;
pub use storm::{Error, Result};
pub use tiberius;
pub use to_sql::{ToSql, ToSqlNull};
pub use transaction_scoped::TransactionScoped;
pub use upsert_builder::{_upsert, UpsertBuilder};

pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

pub fn create_provider_container_from_env(env_var: &str, name: &str) -> Result<ProviderContainer> {
    let factory = MssqlFactory::from_env(env_var)?;

    let mut container = ProviderContainer::new();
    container.register(name, factory);

    Ok(container)
}
