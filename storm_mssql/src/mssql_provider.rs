use crate::{execute::ExecuteArgs, Client, ClientFactory, Execute, Parameter, QueryRows, ToSql};
use futures_util::TryStreamExt;
use std::{
    borrow::Cow,
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc,
    },
};
use storm::{provider, BoxFuture, Error, Result};
use tiberius::{QueryItem, Row};
use tokio::sync::{Mutex, MutexGuard};
use tracing::{instrument, Instrument};

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
}

impl Execute for MssqlProvider {
    #[instrument(
        level = "debug",
        name = "MssqlProvider::execute",
        skip(self, params),
        err
    )]
    fn execute_with_args<'a, S>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
        args: ExecuteArgs,
    ) -> BoxFuture<'a, Result<u64>>
    where
        S: ?Sized + Debug + Into<Cow<'a, str>> + Send + 'a,
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

            let count = client
                .execute(statement, &output)
                .await
                .map_err(Error::std)?
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
                p.state().await;
            }
            .instrument(tracing::debug_span!(parent: span, "rollback_transaction")),
        );
    }

    fn commit(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move { self.state().await.commit().await })
    }
}

impl QueryRows for MssqlProvider {
    #[instrument(
        level = "debug",
        name = "MssqlProvider::query_rows",
        skip(self, mapper, params),
        err
    )]
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
        S: ?Sized + Debug + for<'b> Into<Cow<'b, str>> + Send + 'a,
    {
        Box::pin(async move {
            let mut intermediate = Vec::new();
            let mut output = Vec::new();

            adapt_params(params, &mut intermediate, &mut output);

            let mut coll = C::default();
            let mut vec = Vec::with_capacity(10);
            let mut guard = self.state().await;

            let mut client = match use_transaction {
                true => guard.transaction().await,
                false => guard.client().await,
            }?;

            let mut results = client.query(statement, &output).await.map_err(Error::std)?;

            while let Some(item) = results.try_next().await.map_err(Error::std)? {
                let row = match item {
                    QueryItem::Metadata(_) => continue,
                    QueryItem::Row(row) => row,
                };

                vec.push(mapper(row)?);

                if vec.len() == 10 {
                    coll.extend(vec.drain(..));
                }
            }

            if !vec.is_empty() {
                coll.extend(vec);
            }

            // complete the work (making sure the client can take another query).
            results.into_results().await.map_err(Error::std)?;

            match use_transaction {
                true => guard.transaction = Some(client),
                false => guard.client = Some(client),
            }

            Ok(coll)
        })
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
            client.simple_query(statement).await.map_err(Error::Mssql)?;

            if self.client.is_none() {
                self.client = Some(client);
            }
        }

        Ok(())
    }

    async fn client(&mut self) -> Result<Client> {
        match self.client.take() {
            Some(c) => Ok(c),
            None => self.create_client().await,
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
                    .map_err(Error::Mssql)?;

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
