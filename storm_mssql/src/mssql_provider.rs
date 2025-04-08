use crate::{execute::ExecuteArgs, Client, ClientFactory, Execute, Parameter, QueryRows, ToSql};
use chrono::NaiveDateTime;
use futures::{Stream, StreamExt, TryStreamExt};
use std::{
    borrow::Cow,
    fmt::Debug,
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc,
    },
    task::{Context, Poll},
    time::Duration,
};
use storm::{provider, BoxFuture, Error, Result};
use tiberius::Row;
use tokio::sync::{Mutex, MutexGuard};
use tracing::info;

pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(90);

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

    async fn query_rows_imp<'a, 'b, M, R, C>(
        &'a self,
        sql: &'b str,
        params: &'b [&'b (dyn ToSql)],
        mut mapper: M,
        use_transaction: bool,
    ) -> Result<C>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send + 'a,
        R: Send,
        'a: 'b,
    {
        let mut conn = QueryConn::new(self, use_transaction).await?;
        let mut query = conn.query(sql, params).await?;
        let mut vec = Vec::with_capacity(10);
        let mut coll = C::default();

        while let Some(row) = query.try_next().await? {
            vec.push(mapper(row)?);

            if vec.len() == 10 {
                #[allow(clippy::iter_with_drain)]
                coll.extend(vec.drain(..));
            }
        }

        if !vec.is_empty() {
            coll.extend(vec);
        }

        query.complete().await?;
        conn.complete();

        Ok(coll)
    }

    pub async fn set_client_lock_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.0.state.lock().await.set_lock_timeout(timeout).await
    }
}

impl Execute for MssqlProvider {
    fn execute_with_args<'a, S>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
        args: ExecuteArgs,
    ) -> BoxFuture<'a, Result<u64>>
    where
        S: Debug + Into<Cow<'a, str>> + Send + 'a,
    {
        Box::pin(async move {
            let mut intermediate = Vec::new();
            let mut output = Vec::new();

            adapt_params(params, &mut intermediate, &mut output);

            let mut client;
            let client_ref;
            let mut guard = self.state().await;

            if args.use_transaction {
                client = guard.transaction().await?;
                client_ref = &mut guard.transaction;
            } else {
                client = guard.client().await?;
                client_ref = &mut guard.client;
            };

            let count = match client.execute(statement, &output).await.map(|v| v.total()) {
                Ok(count) => count,
                Err(e) => {
                    let _ = trace_deadlock(&mut client).await;
                    return Err(e.into());
                }
            };

            *client_ref = Some(client);
            Ok(count)
        })
    }
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
        params: &'a [&'a (dyn ToSql)],
        mut mapper: M,
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
            let mut count = 0;

            loop {
                let r = self
                    .query_rows_imp(&sql, params, &mut mapper, use_transaction)
                    .await;

                if r.is_ok() || count > 5 {
                    return r;
                }

                count += 1;
            }
        })
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
        params: &'c [&'c (dyn ToSql)],
    ) -> Result<QueryStream<'b>>
    where
        'b: 'c,
    {
        let mut intermediate = Vec::new();
        let mut output = Vec::new();

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

fn adapt_params<'a>(
    input: &'a [&dyn ToSql],
    intermediate: &'a mut Vec<Parameter<'a>>,
    output: &mut Vec<&'a dyn tiberius::ToSql>,
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
