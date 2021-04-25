mod client_factory;
mod execute;
mod filter_sql;
mod from_sql;
mod mssql_factory;
mod mssql_provider;
mod parameter;
mod query_rows;
mod save_entity_part;
mod to_sql;
mod upsert_builder;

pub use client_factory::ClientFactory;
pub use execute::Execute;
pub use filter_sql::*;
pub use from_sql::FromSql;
pub use mssql_factory::MssqlFactory;
pub use mssql_provider::MssqlProvider;
pub use parameter::Parameter;
pub use query_rows::QueryRows;
pub use save_entity_part::SaveEntityPart;
use storm::ProviderContainer;
pub use storm::{Error, Result};
pub use tiberius;
pub use to_sql::{ToSql, ToSqlNull};
pub use upsert_builder::UpsertBuilder;

pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

pub fn create_provider_container_from_env(env_var: &str, name: &str) -> Result<ProviderContainer> {
    let factory = MssqlFactory::from_env(env_var)?;

    let mut container = ProviderContainer::new();
    container.register(name, factory);

    Ok(container)
}
