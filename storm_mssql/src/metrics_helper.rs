use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use tracing::instrument;

#[doc(hidden)]
pub fn delete_wrap<'a, F, T, E>(
    f: F,
    table: &'static str,
) -> Pin<Box<dyn Future<Output = F::Output> + Send + 'a>>
where
    F: Future<Output = Result<T, E>> + Send + 'a,
    E: Display,
{
    op_wrap(f, table, "delete")
}

#[doc(hidden)]
pub fn load_wrap<'a, F, T, E>(
    f: F,
    table: &'static str,
) -> Pin<Box<dyn Future<Output = F::Output> + Send + 'a>>
where
    F: Future<Output = Result<T, E>> + Send + 'a,
    E: Display,
{
    op_wrap(f, table, "load")
}

#[allow(clippy::redundant_async_block)]
#[doc(hidden)]
#[instrument(name = "storm_mssql::op", skip(f), err)]
fn op_wrap<'a, F, T, E>(
    f: F,
    table: &'static str,
    op: &'static str,
) -> Pin<Box<dyn Future<Output = F::Output> + Send + 'a>>
where
    F: Future<Output = Result<T, E>> + Send + 'a,
    E: Display,
{
    Box::pin(async move {
        #[cfg(feature = "telemetry")]
        {
            let d = std::time::Instant::now();
            let r = f.await;
            let d = d.elapsed();

            metrics::counter!("storm_mssql_count", "table" => table, "op" => op).increment(1);
            metrics::counter!("storm_mssql_ms", "table" => table, "op" => op)
                .increment(d.as_millis() as u64);

            r
        }

        #[cfg(not(feature = "telemetry"))]
        {
            f.await
        }
    })
}

#[doc(hidden)]
pub fn upsert_wrap<'a, F, T, E>(
    f: F,
    table: &'static str,
) -> Pin<Box<dyn Future<Output = F::Output> + Send + 'a>>
where
    F: Future<Output = Result<T, E>> + Send + 'a,
    E: Display,
{
    op_wrap(f, table, "upsert")
}
