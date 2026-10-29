use crate::{Client, ClientFactory, Execute, Parameter, QueryRows, ToSql, execute::ExecuteArgs};
use chrono::NaiveDateTime;
use futures::{Stream, StreamExt, TryStreamExt};
use smallvec::SmallVec;
use std::{
    borrow::Cow,
    fmt::Debug,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
    task::{Context, Poll},
    time::Duration,
};
use storm::{BoxFuture, Error, Result, provider};
use tiberius::Row;
use tokio::sync::{Mutex, MutexGuard};
use tracing::info;

pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(90);

/// Rows are handed to the collection in batches of this size.
/// Live benchmarks showed no measurable difference between 10, 64 and 256.
pub(crate) const ROW_BATCH: usize = 10;

pub struct MssqlProvider(Arc<Inner>);

impl MssqlProvider {
    pub fn new<F: ClientFactory>(client_factory: F) -> Self {
        (Box::new(client_factory) as Box<dyn ClientFactory>).into()
    }

    async fn state(&self) -> MutexGuard<'_, State> {
        let mut guard = self.0.state.lock().await;

        if self.0.cancel_transaction.swap(false, Relaxed) {
            let _ = guard.cancel().await;
        }

        guard
    }

    /// Creates a new [Client](Client) instance.
    /// # Safety
    /// This operation is safe but the returning client is not constrained by the lock and can modify the database without storm's knowledge.
    pub async unsafe fn create_client(&self) -> Result<Client> {
        self.0.state.lock().await.create_client().await
    }

    /// Creates a new [Client](Client) instance.
    /// # Safety
    /// This operation is safe but the returning client is not constrained by the lock and can modify the database without storm's knowledge.
    pub async unsafe fn get_transaction_client(&self) -> Option<MssqlTransactionGuard<'_>> {
        let guard = self.0.state.lock().await;
        guard
            .transaction
            .is_some()
            .then_some(MssqlTransactionGuard(guard))
    }

    /// Runs the query, retrying on failure, and hands each row to `sink`.
    ///
    /// Not generic so the row loop and the retries are compiled once instead of once per
    /// entity and collection.
    pub(crate) async fn query_rows_dyn(
        &self,
        sql: &str,
        params: &[&dyn ToSql],
        use_transaction: bool,
        sink: &mut dyn RowSink,
    ) -> Result<()> {
        let mut count = 0;

        loop {
            let r = self
                .query_rows_once(sql, params, use_transaction, sink)
                .await;

            if r.is_ok() || count > 5 {
                return r;
            }

            count += 1;
            sink.reset();
        }
    }

    async fn query_rows_once(
        &self,
        sql: &str,
        params: &[&dyn ToSql],
        use_transaction: bool,
        sink: &mut dyn RowSink,
    ) -> Result<()> {
        let mut conn = QueryConn::new(self, use_transaction).await?;
        let mut query = conn.query(sql, params).await?;

        while let Some(row) = query.try_next().await? {
            sink.row(row)?;
        }

        sink.end();
        query.complete().await?;
        conn.complete();

        Ok(())
    }

    pub async fn set_client_lock_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.0.state.lock().await.set_lock_timeout(timeout).await
    }
}

impl Execute for MssqlProvider {
    fn execute_with_args<'a, S>(
        &'a self,
        statement: S,
        params: &'a [&'a dyn ToSql],
        args: ExecuteArgs,
    ) -> BoxFuture<'a, Result<u64>>
    where
        S: Debug + Into<Cow<'a, str>> + Send + 'a,
    {
        Box::pin(self.execute_imp(statement.into(), params, args))
    }
}

impl MssqlProvider {
    // Non-generic so the body is compiled once instead of once per statement type.
    async fn execute_imp<'a>(
        &'a self,
        statement: Cow<'a, str>,
        params: &'a [&'a dyn ToSql],
        args: ExecuteArgs,
    ) -> Result<u64> {
        let mut intermediate = SmallVec::new();
        let mut output = SmallVec::new();

        adapt_params(params, &mut intermediate, &mut output);

        self.execute_tds_with(&statement, &output, args.use_transaction)
            .await
    }

    /// Executes in the transaction with parameters already adapted for tiberius.
    pub(crate) async fn execute_tds(
        &self,
        sql: &str,
        params: &[&dyn tiberius::ToSql],
    ) -> Result<u64> {
        self.execute_tds_with(sql, params, true).await
    }

    async fn execute_tds_with(
        &self,
        sql: &str,
        params: &[&dyn tiberius::ToSql],
        use_transaction: bool,
    ) -> Result<u64> {
        let mut client;
        let client_ref;
        let mut guard = self.state().await;

        if use_transaction {
            client = guard.transaction().await?;
            client_ref = &mut guard.transaction;
        } else {
            client = guard.client().await?;
            client_ref = &mut guard.client;
        };

        let count = match client.execute(sql, params).await.map(|v| v.total()) {
            Ok(count) => count,
            Err(e) => {
                let _ = trace_deadlock(&mut client).await;
                return Err(e.into());
            }
        };

        *client_ref = Some(client);
        Ok(count)
    }

    /// Runs `sql` once in the transaction and returns its last row.
    ///
    /// Unlike [QueryRows], it is not retried, so it is safe for statements that modify data.
    pub(crate) async fn query_last_tds(
        &self,
        sql: &str,
        params: &[&dyn tiberius::ToSql],
    ) -> Result<Option<Row>> {
        let mut guard = self.state().await;
        let mut client = guard.transaction().await?;

        match query_last(&mut client, sql, params).await {
            Ok(row) => {
                guard.transaction = Some(client);
                Ok(row)
            }
            Err(e) => {
                let _ = trace_deadlock(&mut client).await;
                Err(e)
            }
        }
    }
}

async fn query_last(
    client: &mut Client,
    sql: &str,
    params: &[&dyn tiberius::ToSql],
) -> Result<Option<Row>> {
    let mut stream = QueryStream(client.query(sql, params).await?);
    let mut last = None;

    while let Some(row) = stream.try_next().await? {
        last = Some(row);
    }

    Ok(last)
}

struct Inner {
    cancel_transaction: AtomicBool,
    state: Mutex<State>,
}

impl From<Box<dyn ClientFactory>> for MssqlProvider {
    fn from(factory: Box<dyn ClientFactory>) -> Self {
        Self(Arc::new(Inner {
            cancel_transaction: Default::default(),
            state: Mutex::new(State::new(factory)),
        }))
    }
}

impl provider::Provider for MssqlProvider {
    fn cancel(&self) {
        self.0.cancel_transaction.store(true, Relaxed);

        let p = Self(Arc::clone(&self.0));

        tokio::spawn(async move {
            let _ = p.state().await;
        });
    }

    fn commit(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move { self.state().await.commit().await })
    }
}

impl QueryRows for MssqlProvider {
    fn query_rows<'a, S, M, R, C>(
        &'a self,
        statement: S,
        params: &'a [&'a dyn ToSql],
        mapper: M,
        use_transaction: bool,
    ) -> BoxFuture<'a, Result<C>>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send + 'a,
        R: Send,
        S: Debug + for<'b> Into<Cow<'b, str>> + Send + 'a,
    {
        let sql = statement.into();

        Box::pin(async move {
            let mut sink = Collect {
                coll: C::default(),
                mapper,
                rows: Vec::with_capacity(ROW_BATCH),
            };

            self.query_rows_dyn(&sql, params, use_transaction, &mut sink)
                .await?;

            Ok(sink.coll)
        })
    }
}

/// Receives the rows of a query.
pub(crate) trait RowSink: Send {
    fn row(&mut self, row: Row) -> Result<()>;

    /// Called after the last row, before the query completes.
    fn end(&mut self);

    /// Called before the query is retried; drops what the failed attempt received.
    fn reset(&mut self);
}

/// Maps the rows and hands them to a collection in batches.
struct Collect<M, R, C> {
    coll: C,
    mapper: M,
    rows: Vec<R>,
}

impl<M, R, C> RowSink for Collect<M, R, C>
where
    C: Default + Extend<R> + Send,
    M: FnMut(Row) -> Result<R> + Send,
    R: Send,
{
    fn row(&mut self, row: Row) -> Result<()> {
        self.rows.push((self.mapper)(row)?);

        if self.rows.len() == ROW_BATCH {
            self.coll.extend(self.rows.drain(..));
        }

        Ok(())
    }

    fn end(&mut self) {
        if !self.rows.is_empty() {
            self.coll.extend(self.rows.drain(..));
        }
    }

    fn reset(&mut self) {
        self.rows.clear();
        self.coll = C::default();
    }
}

struct QueryConn<'a> {
    client: Client,
    guard: MutexGuard<'a, State>,
    use_transaction: bool,
}

impl<'a> QueryConn<'a> {
    async fn new(provider: &'a MssqlProvider, use_transaction: bool) -> Result<QueryConn<'a>> {
        let mut guard = provider.state().await;

        let client = match use_transaction {
            true => guard.transaction().await,
            false => guard.client().await,
        }?;

        Ok(Self {
            client,
            guard,
            use_transaction,
        })
    }

    fn complete(mut self) {
        match self.use_transaction {
            true => self.guard.transaction = Some(self.client),
            false => self.guard.client = Some(self.client),
        }
    }

    async fn query<'b, 'c>(
        &'b mut self,
        sql: &'c str,
        params: &'c [&'c dyn ToSql],
    ) -> Result<QueryStream<'b>>
    where
        'b: 'c,
    {
        let mut intermediate = SmallVec::new();
        let mut output = SmallVec::new();

        adapt_params(params, &mut intermediate, &mut output);

        let stream = self.client.query(sql, &output[..]).await?;

        Ok(QueryStream(stream))
    }
}

async fn trace_deadlock(client: &mut Client) -> Result<()> {
    const SQL: &str = r#"
        SELECT
            a.session_id,
            a.start_time,
            a.[status],
            a.command,
            a.blocking_session_id,
            a.wait_type,
            a.wait_time,
            a.open_transaction_count,
            a.transaction_id,
            a.total_elapsed_time,
            definition = CAST(b.text AS VARCHAR(MAX))
        FROM
            SYS.DM_EXEC_REQUESTS a
            CROSS APPLY sys.dm_exec_sql_text(a.sql_handle) b
        WHERE
            a.session_id != @@SPID
            AND a.database_id = DB_ID()
            AND blocking_session_id != 0
    "#;

    let deadlocks = client
        .simple_query(SQL)
        .await?
        .into_first_result()
        .await?
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "session_id": row.get::<i16, _>(0),
                "start_time": row.get::<NaiveDateTime, _>(1),
                "status": row.get::<&str, _>(2).unwrap_or_default(),
                "command": row.get::<&str, _>(3).unwrap_or_default(),
                "blocking_session_id": row.get::<i16, _>(4),
                "wait_type": row.get::<&str, _>(5).unwrap_or_default(),
                "wait_time": row.get::<i32, _>(6),
                "open_transaction_count": row.get::<i32, _>(7),
                "transaction_id": row.get::<i64, _>(8),
                "total_elapsed_time": row.get::<i32, _>(9),
                "definition": row.get::<&str, _>(10).unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();

    if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!(deadlocks)) {
        info!(json = json, "potential sql deadlocks");
    }

    Ok(())
}

struct QueryStream<'a>(tiberius::QueryStream<'a>);

impl QueryStream<'_> {
    async fn complete(self) -> Result<()> {
        self.0.into_results().await?;
        Ok(())
    }
}

impl Stream for QueryStream<'_> {
    type Item = Result<tiberius::Row>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            return match self.0.poll_next_unpin(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Some(Ok(tiberius::QueryItem::Metadata(_)))) => continue,
                Poll::Ready(Some(Ok(tiberius::QueryItem::Row(r)))) => Poll::Ready(Some(Ok(r))),
                Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e.into()))),
                Poll::Ready(None) => Poll::Ready(None),
            };
        }
    }
}

struct State {
    client: Option<Client>,
    factory: Box<dyn ClientFactory>,
    lock_timeout: Option<Duration>,
    transaction: Option<Client>,
}

impl State {
    fn new(factory: Box<dyn ClientFactory>) -> Self {
        Self {
            client: None,
            factory,
            lock_timeout: Some(DEFAULT_LOCK_TIMEOUT),
            transaction: None,
        }
    }

    async fn cancel(&mut self) -> Result<()> {
        self.cancel_or_commit("ROLLBACK").await
    }

    async fn cancel_or_commit(&mut self, statement: &'static str) -> Result<()> {
        if let Some(mut client) = self.transaction.take() {
            let r = client.simple_query(statement).await.map_err(Error::Mssql);

            #[cfg(feature = "telemetry")]
            {
                metrics::gauge!("storm_mssql_transaction_count").decrement(1.0);
            }

            r?;

            if self.client.is_none() {
                self.client = Some(client);
            }
        }

        Ok(())
    }

    async fn client(&mut self) -> Result<Client> {
        match self.client.take() {
            Some(c) => Ok(c),
            None => {
                if let Some(client) = self
                    .factory
                    .under_transaction()
                    .then(|| self.transaction.take())
                    .flatten()
                {
                    return Ok(client);
                }

                self.create_client().await
            }
        }
    }

    async fn commit(&mut self) -> Result<()> {
        self.cancel_or_commit("COMMIT").await
    }

    async fn create_client(&self) -> Result<Client> {
        let mut client = self.factory.create_client().await?;

        set_client_lock_timeout(&mut client, self.lock_timeout).await?;

        Ok(client)
    }

    async fn set_lock_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        if self.lock_timeout != timeout {
            let mut client = self.client.take();

            if let Some(client) = client.as_mut() {
                set_client_lock_timeout(client, timeout).await?;
            }

            if let Some(client) = self.transaction.as_mut() {
                set_client_lock_timeout(client, timeout).await?;
            }

            self.client = client;
        }

        Ok(())
    }

    async fn transaction(&mut self) -> Result<Client> {
        match self.transaction.take() {
            Some(t) => Ok(t),
            None => {
                let mut client = self.client().await?;

                let r = client
                    .simple_query("BEGIN TRAN")
                    .await
                    .map_err(Error::Mssql);

                #[cfg(feature = "telemetry")]
                {
                    metrics::gauge!("storm_mssql_transaction_count").increment(1.0);
                }

                r?;

                Ok(client)
            }
        }
    }
}

async fn set_client_lock_timeout(client: &mut Client, timeout: Option<Duration>) -> Result<()> {
    client
        .simple_query(format!(
            "SET LOCK_TIMEOUT {};",
            timeout.map_or(-1, |d| d.as_millis() as i128)
        ))
        .await?;

    Ok(())
}

fn adapt_params<'a, 'b>(
    input: &'a [&dyn ToSql],
    intermediate: &'b mut SmallVec<[Parameter<'a>; 8]>,
    output: &mut SmallVec<[&'b dyn tiberius::ToSql; 8]>,
) {
    intermediate.extend(input.iter().map(|p| Parameter(p.to_sql())));
    output.extend(intermediate.iter().map(|p| p as &dyn tiberius::ToSql));
}

pub struct MssqlTransactionGuard<'a>(MutexGuard<'a, State>);

impl Deref for MssqlTransactionGuard<'_> {
    type Target = Client;

    #[allow(clippy::expect_used)]
    fn deref(&self) -> &Self::Target {
        self.0.transaction.as_ref().expect("Transaction")
    }
}

impl DerefMut for MssqlTransactionGuard<'_> {
    #[allow(clippy::expect_used)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.transaction.as_mut().expect("Transaction")
    }
}
