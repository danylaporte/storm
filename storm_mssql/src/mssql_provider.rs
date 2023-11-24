use crate::{
    execute::ExecuteArgs, Client, ClientFactory, Error, Execute, Parameter, QueryRows, ToSql,
};
use futures::{Stream, StreamExt, TryStreamExt};
use std::{
    ops::{Deref, DerefMut},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc,
    },
    task::{Context, Poll},
};
use storm::{provider, BoxFuture, Result};
use tiberius::Row;
use tokio::sync::{Mutex, MutexGuard};
use tracing::{info_span, instrument, Instrument};

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
}

impl Execute for MssqlProvider {
    #[instrument(name = "MssqlProvider::execute", skip_all, err)]
    fn execute_with_args<'a>(
        &'a self,
        statement: String,
        params: &'a [&'a (dyn ToSql)],
        args: ExecuteArgs,
    ) -> BoxFuture<'a, Result<u64>> {
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

            let sql = statement.clone();

            let count = client
                .execute(statement, &output)
                .await
                .map_err(|source| Error::Query { source, sql })?
                .total();

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

        let span = tracing::Span::current();
        let p = Self(Arc::clone(&self.0));

        tokio::spawn(
            async move {
                let _ = p.state().await;
            }
            .instrument(info_span!(parent: span, "rollback_transaction")),
        );
    }

    fn commit(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move { self.state().await.commit().await })
    }
}

impl QueryRows for MssqlProvider {
    #[instrument(name = "MssqlProvider::query", skip_all, err)]
    fn query_rows<'a, M, R, C>(
        &'a self,
        sql: String,
        params: &'a [&'a (dyn ToSql)],
        mut mapper: M,
        use_transaction: bool,
    ) -> BoxFuture<'a, Result<C>>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send + 'a,
        R: Send,
    {
        Box::pin(async move {
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

    async fn query<'b>(
        &'b mut self,
        statement: String,
        params: &'b [&'b (dyn ToSql)],
    ) -> Result<QueryStream<'b>> {
        let mut intermediate = Vec::new();
        let mut output = Vec::new();

        adapt_params(params, &mut intermediate, &mut output);

        let sql = statement.clone();

        let stream = self
            .client
            .query(statement, &output[..])
            .await
            .map_err(|source| Error::Query { source, sql })?;

        Ok(QueryStream(stream))
    }
}

struct QueryStream<'a>(tiberius::QueryStream<'a>);

impl<'a> QueryStream<'a> {
    async fn complete(self) -> Result<()> {
        self.0.into_results().await.map_err(Error::unknown)?;
        Ok(())
    }
}

impl<'a> Stream for QueryStream<'a> {
    type Item = Result<tiberius::Row>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            return match self.0.poll_next_unpin(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Some(Ok(tiberius::QueryItem::Metadata(_)))) => continue,
                Poll::Ready(Some(Ok(tiberius::QueryItem::Row(r)))) => Poll::Ready(Some(Ok(r))),
                Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(Error::unknown(e).into()))),
                Poll::Ready(None) => Poll::Ready(None),
            };
        }
    }
}

struct State {
    client: Option<Client>,
    factory: Box<dyn ClientFactory>,
    transaction: Option<Client>,
}

impl State {
    fn new(factory: Box<dyn ClientFactory>) -> Self {
        Self {
            client: None,
            factory,
            transaction: None,
        }
    }

    async fn cancel(&mut self) -> Result<()> {
        self.cancel_or_commit("ROLLBACK").await
    }

    async fn cancel_or_commit(&mut self, statement: &'static str) -> Result<()> {
        if let Some(mut client) = self.transaction.take() {
            client
                .simple_query(statement)
                .await
                .map_err(Error::unknown)?;

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
        self.factory.create_client().await
    }

    async fn transaction(&mut self) -> Result<Client> {
        match self.transaction.take() {
            Some(t) => Ok(t),
            None => {
                let mut client = self.client().await?;

                client
                    .simple_query("BEGIN TRAN")
                    .await
                    .map_err(Error::unknown)?;

                Ok(client)
            }
        }
    }
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

impl<'a> Deref for MssqlTransactionGuard<'a> {
    type Target = Client;

    #[allow(clippy::expect_used)]
    fn deref(&self) -> &Self::Target {
        self.0.transaction.as_ref().expect("Transaction")
    }
}

impl<'a> DerefMut for MssqlTransactionGuard<'a> {
    #[allow(clippy::expect_used)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.transaction.as_mut().expect("Transaction")
    }
}
