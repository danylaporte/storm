#![allow(clippy::unwrap_used)]

use storm::{Entity, Error, Result, provider::ProviderContainer};
use storm_mssql::{MssqlFactory, MssqlProvider, QueryRows};
use tiberius::Config;

const SQL: &str = "SELECT v FROM (VALUES (1), (2), (3), (4), (5), (6), (7), (8), (9), (10), (11), (12)) t(v) ORDER BY v";

fn provider() -> ProviderContainer {
    let mut config = Config::default();
    config.database("master");
    #[cfg(target_os = "windows")]
    config.authentication(tiberius::AuthMethod::Integrated);
    config.trust_cert();

    let mut provider = ProviderContainer::new();
    provider.register("", MssqlFactory(config));

    provider
}

/// Fails on the 11th row of the first attempt, after a full batch was handed to the collection.
fn failing_once() -> impl FnMut(tiberius::Row) -> Result<i32> + Send {
    let mut calls = 0;

    move |row| {
        calls += 1;

        if calls == 11 {
            return Err(Error::ColumnNull);
        }

        Ok(row.get::<i32, _>(0).unwrap())
    }
}

#[async_cell_lock::tokio_test]
async fn retry_drops_rows_of_failed_attempt() -> Result<()> {
    let container = provider();
    let provider = container.provide::<MssqlProvider>("").await?;
    let mut mapper = failing_once();

    let rows: Vec<i32> = provider
        .query_rows(SQL.to_owned(), &[], &mut mapper, false)
        .await?;

    assert_eq!(rows, (1..=12).collect::<Vec<_>>());
    Ok(())
}

#[async_cell_lock::tokio_test]
async fn load_rows_retry_drops_rows_of_failed_attempt() -> Result<()> {
    let container = provider();
    let provider = container.provide::<MssqlProvider>("").await?;
    let mut mapper = failing_once();
    let mut rows: Vec<(i32, Row)> = Vec::new();

    storm_mssql::_load_rows(
        provider,
        SQL,
        &[],
        |row| mapper(row).map(|v| (v, Row)),
        false,
        &mut rows as &mut dyn storm_mssql::load_sink::LoadSink<Row>,
    )
    .await?;

    assert_eq!(
        rows.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        (1..=12).collect::<Vec<_>>()
    );
    Ok(())
}

#[derive(Debug, PartialEq)]
struct Row;

impl Entity for Row {
    type Key = i32;
}
