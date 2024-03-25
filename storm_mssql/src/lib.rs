mod client_factory;
mod entity_diff;
mod execute;
mod field_diff;
mod filter_sql;
mod from_sql;
#[doc(hidden)]
pub mod metrics_helper;
mod mssql_factory;
mod mssql_meta;
mod mssql_provider;
mod parameter;
mod query_rows;
mod save_entity_part;
mod to_sql;
mod transaction_scoped;
mod upsert_builder;


use std::pin::Pin;

pub use client_factory::ClientFactory;
pub use entity_diff::*;
pub use execute::*;
pub use field_diff::*;
pub use filter_sql::*;
pub use from_sql::{FromSql, _macro_load_field};
pub use mssql_factory::MssqlFactory;
pub use mssql_meta::MssqlMeta;
pub use mssql_provider::{MssqlProvider, MssqlTransactionGuard};
pub use parameter::{into_column_data_static, Parameter};
pub use query_rows::QueryRows;
pub use save_entity_part::SaveEntityPart;
pub use serde_json;
use storm::ProviderContainer;
pub use storm::{Error, Result};
pub use tiberius;
pub use to_sql::{ToSql, ToSqlNull};
pub use transaction_scoped::TransactionScoped;
pub use upsert_builder::UpsertBuilder;
use std::future::Future;

pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;

pub fn create_provider_container_from_env(env_var: &str, name: &str) -> Result<ProviderContainer> {
    let factory = MssqlFactory::from_env(env_var)?;

    let mut container = ProviderContainer::new();
    container.register(name, factory);

    Ok(container)
}

type MaxLength = usize;

#[doc(hidden)]
pub fn test_entity<'a>(provider: &'a str, table: &'a str, translated_table: &'a str, expected: &'a [(&'a str, MaxLength)]) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    Box::pin(async move {
        let container = crate::create_provider_container_from_env("DB", provider).expect("container");
        let provider = container.provide::<MssqlProvider>(provider).await.expect("provider");
        

        fn split_table_schema(table: &str) -> (&str, &str) {
            let mut iter = table.split(".").map(|s| s.trim_matches('[').trim_matches(']')).filter(|s| !s.is_empty());

            match (iter.next(), iter.next()) {
                (None, Some(t)) | (Some(t), None) => ("dbo", t),
                (Some(s), Some(t)) => (s, t),
                _ => panic!("table invalid"),
            }
        }

        let (schema1, table1) = split_table_schema(table);
        let (schema2, table2) = if translated_table.is_empty() { ("dbo", table1) } else { split_table_schema(translated_table) };

        let sql = r"
        SELECT
            c.[name],
            CAST(c.max_length as int),
            CAST((CASE WHEN c.system_type_id IN (99, 231, 239) THEN 1 ELSE 0 END) as BIT) IsNChar
        FROM
            sys.all_columns c
            INNER JOIN sys.tables t
            ON c.object_id = t.object_id
        WHERE
            (t.name = @P1 AND t.schema_id = SCHEMA_ID(@P2))
            OR (t.name = @P3 AND t.schema_id = SCHEMA_ID(@P4))";

        let mut actual: Vec<(String, i32)> = provider.query_rows(sql.to_string(), &[&table1, &schema1, &table2, &schema2], |row| {
            let is_nchar = row.get::<bool, _>(2).unwrap_or_default();
            let nchar_div = if is_nchar { 2 } else { 1 };
            let mut max_length = row.get::<i32, _>(1).unwrap_or_default() / nchar_div;

            if max_length == -1 {
                max_length = 0
            }

            Ok((
                row.get::<&str, _>(0).unwrap_or_default().to_lowercase(),
                max_length,
            ))
        }, false).await.expect("rows");

        actual.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        for expected in expected {
            let n = expected.0.to_lowercase();
            let n = n.trim_matches('[').trim_matches(']');

            let Some(actual) = actual.iter().find(|t| t.0 == n) else {
                panic!("column {n} not found");
            };

            if expected.1 > 0 && expected.1 as i32 != actual.1 {
                panic!("field maxlength {} differ, actual: {}, expected: {}", &expected.0, actual.1, expected.1);
            }
        }
    })
}
