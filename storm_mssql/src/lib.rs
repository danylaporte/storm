mod client_factory;
mod entity_diff;
mod error;
mod execute;
mod field_diff;
mod filter_sql;
pub mod from_sql;
mod mssql_factory;
mod mssql_meta;
mod mssql_provider;
mod parameter;
mod query_rows;
mod save_entity_part;
mod to_sql;
mod transaction_scoped;
mod upsert_builder;

pub use client_factory::ClientFactory;
pub use entity_diff::*;
pub use error::{Error, FieldDiffError};
pub use execute::*;
pub use field_diff::*;
pub use filter_sql::*;
pub use from_sql::{FromSql, FromSqlError, _macro_load_field};
pub use mssql_factory::MssqlFactory;
pub use mssql_meta::MssqlMeta;
pub use mssql_provider::{MssqlProvider, MssqlTransactionGuard};
pub use parameter::{into_column_data_static, Parameter};
pub use query_rows::QueryRows;
pub use save_entity_part::SaveEntityPart;
pub use serde_json;
use storm::ProviderContainer;
pub use tiberius;
pub use to_sql::{ToSql, ToSqlNull};
pub use transaction_scoped::TransactionScoped;
pub use upsert_builder::UpsertBuilder;

pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

pub fn create_provider_container_from_env_with_trust(
    env_var: &str,
    name: &str,
    trust: bool,
) -> storm::Result<ProviderContainer> {
    let factory = MssqlFactory::from_env_with_trust(env_var, trust)?;

    let mut container = ProviderContainer::new();
    container.register(name, factory);

    Ok(container)
}
