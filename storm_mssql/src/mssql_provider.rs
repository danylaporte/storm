use crate::{Client, ClientFactory, Execute, Parameter, QueryRows, ToSql};
use futures_util::TryStreamExt;
use std::{
    borrow::Cow,
    fmt::Debug,
    mem::replace,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};
use storm::{provider, BoxFuture, Error, Result};
use tiberius::Row;
use tokio::sync::{Mutex, MutexGuard};
use tracing::instrument;

pub struct MssqlProvider {
    cancel_transaction: AtomicBool,
    client_factory: Box<dyn ClientFactory>,
    state: Mutex<Option<State>>,
}

impl MssqlProvider {
    pub fn new<F: ClientFactory>(client_factory: F) -> Self {
        (Box::new(client_factory) as Box<dyn ClientFactory>).into()
    }

    async fn client(&self) -> Result<(MutexGuard<'_, Option<State>>, State)> {
        let mut guard = self.state().await?;

        let state = match guard.take() {
            Some(state) => state,
            None => State {
                client: self.client_factory.create_client().await?,
                in_transaction: false,
            },
        };

        Ok((guard, state))
    }

    async fn state(&self) -> Result<MutexGuard<'_, Option<State>>> {
        let mut guard = self.state.lock().await;

        if self.cancel_transaction.swap(false, Relaxed) {
            if let Some(mut state) = guard.take() {
                state.cancel().await?;
                *guard = Some(state);
            }
        }

        Ok(guard)
    }
}

impl Execute for MssqlProvider {
    #[instrument(name = "MssqlProvider::execute", skip(self, params), err)]
    fn execute<'a, S>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
    ) -> BoxFuture<'a, Result<u64>>
    where
        S: ?Sized + Debug + Into<Cow<'a, str>> + Send + 'a,
    {
        Box::pin(async move {
            let (mut guard, mut state) = self.client().await?;
            let mut intermediate = Vec::new();
            let mut output = Vec::new();

            adapt_params(params, &mut intermediate, &mut output);

            state.transaction().await?;

            let count = state
                .client
                .execute(statement, &output)
                .await
                .map_err(Error::std)?
                .total();

            *guard = Some(state);
            Ok(count)
        })
    }
}

impl From<Box<dyn ClientFactory>> for MssqlProvider {
    fn from(client_factory: Box<dyn ClientFactory>) -> Self {
        Self {
            cancel_transaction: Default::default(),
            client_factory,
            state: Default::default(),
        }
    }
}

impl provider::Provider for MssqlProvider {
    fn cancel(&self) {
        self.cancel_transaction.store(true, Relaxed);
    }

    fn commit<'a>(&'a self) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let mut guard = self.state().await?;

            if let Some(mut state) = guard.take() {
                state.commit().await?;
                *guard = Some(state);
            }

            Ok(())
        })
    }
}

impl QueryRows for MssqlProvider {
    #[instrument(name = "MssqlProvider::query_rows", skip(self, mapper, params), err)]
    fn query_rows<'a, S, M, R, C>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
        mut mapper: M,
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

            let (mut guard, mut state) = self.client().await?;

            let mut results = state
                .client
                .query(statement, &output)
                .await
                .map_err(Error::std)?;

            let mut coll = C::default();
            let mut vec = Vec::with_capacity(10);

            while let Some(row) = results.try_next().await.map_err(Error::std)? {
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

            *guard = Some(state);
            Ok(coll)
        })
    }
}

struct State {
    client: Client,
    in_transaction: bool,
}

impl State {
    async fn cancel(&mut self) -> Result<()> {
        if replace(&mut self.in_transaction, false) {
            self.client
                .simple_query("ROLLBACK")
                .await
                .map_err(Error::Mssql)?;
        }
        Ok(())
    }

    async fn commit(&mut self) -> Result<()> {
        if replace(&mut self.in_transaction, false) {
            self.client
                .simple_query("COMMIT")
                .await
                .map_err(Error::Mssql)?;
        }

        Ok(())
    }

    async fn transaction(&mut self) -> Result<()> {
        if !self.in_transaction {
            self.client
                .simple_query("BEGIN TRAN")
                .await
                .map_err(Error::Mssql)?;
        }

        Ok(())
    }
}

fn adapt_params<'a>(
    input: &'a [&dyn ToSql],
    intermediate: &'a mut Vec<Parameter<'a>>,
    output: &mut Vec<&'a dyn tiberius::ToSql>,
) {
    intermediate.extend(input.into_iter().map(|p| Parameter(p.to_sql())));
    output.extend(intermediate.iter().map(|p| p as &dyn tiberius::ToSql));
}
