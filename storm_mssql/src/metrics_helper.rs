use std::{fmt::Display, future::Future, pin::Pin};

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
fn op_wrap<'a, F, T, E>(
    f: F,
    #[allow(unused_variables)] table: &'static str,
    #[allow(unused_variables)] op: &'static str,
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
            let e = r.as_ref().err().map(|e| e.to_string());

            counter_impl(table, op, d, e);

            r
        }

        #[cfg(not(feature = "telemetry"))]
        {
            f.await
        }
    })
}

#[cfg(feature = "telemetry")]
fn counter_impl(
    table: &'static str,
    op: &'static str,
    instant: std::time::Instant,
    e: Option<String>,
) {
    let d = instant.elapsed().as_millis();

    if let Some(e) = e {
        let _ = tracing::error_span!(
            "error mssql op",
            dur_ms = d,
            table = table,
            op = op,
            error = e
        )
        .entered();
    } else if d >= 500 {
        let _ = tracing::warn_span!("slow mssql op", dur_ms = d, table = table, op = op).entered();
    }

    metrics::counter!("storm_mssql_count", "table" => table, "op" => op).increment(1);
    metrics::counter!("storm_mssql_ms", "table" => table, "op" => op).increment(d as u64);
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
