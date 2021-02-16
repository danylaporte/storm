use crate::{Client, ClientFactory, Execute, QueryRows};
use async_trait::async_trait;
use futures_util::TryStreamExt;
use std::{
    borrow::Cow,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};
use storm::{provider::Gate, Error, Result};
use tiberius::{Row, ToSql};
use tokio::sync::{Mutex, MutexGuard};
use tracing::instrument;

pub struct MssqlProvider<F> {
    cancel_transaction: AtomicU64,
    client_factory: F,
    gate: Mutex<()>,
    state: Mutex<State>,
}

impl<F> MssqlProvider<F>
where
    F: ClientFactory,
{
    pub(crate) fn new(client_factory: F) -> Self {
        Self {
            cancel_transaction: Default::default(),
            client_factory,
            gate: Mutex::new(()),
            state: Mutex::new(State {
                client: None,
                current_transaction: 0,
                transaction_counter: 1,
            }),
        }
    }

    async fn state_client(&self) -> Result<(MutexGuard<'_, State>, Client)> {
        let mut state = self.state.lock().await;

        let client = match state.client.take() {
            Some(mut client) => {
                if state.current_transaction > 0
                    && state.current_transaction == self.cancel_transaction.load(Relaxed)
                {
                    state.current_transaction = 0;
                    client.simple_query("ROLLBACK").await.map_err(Error::std)?;
                }

                client
            }
            None => {
                state.current_transaction = 0;
                self.client_factory.create_client().await?
            }
        };

        Ok((state, client))
    }

    #[instrument(name = "MssqlProvider::transaction", skip(self), err)]
    pub async fn transaction(&self) -> Result<MssqlTransaction<'_, F>> {
        let (mut state, mut client) = self.state_client().await?;

        if state.current_transaction > 0 {
            state.client = Some(client);
            return Err(Error::AlreadyInTransaction);
        }

        client
            .simple_query("BEGIN TRANS")
            .await
            .map_err(Error::std)?;

        state.client = Some(client);

        let id = state.transaction_counter;

        state.current_transaction = id;
        state.transaction_counter += 1;

        Ok(MssqlTransaction { id, provider: self })
    }
}

#[async_trait]
impl<'a, F> Gate<'a> for MssqlProvider<F>
where
    F: Send + Sync,
{
    type Gate = MutexGuard<'a, ()>;

    async fn gate(&'a self) -> Self::Gate {
        self.gate.lock().await
    }
}

#[async_trait]
impl<F> QueryRows for MssqlProvider<F>
where
    F: ClientFactory,
{
    #[instrument(name = "MssqlProvider::query_rows", skip(self, mapper, params), err)]
    async fn query_rows<S, M, R, C>(
        &self,
        statement: S,
        params: &[&(dyn ToSql)],
        mut mapper: M,
    ) -> Result<C>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send,
        R: Send,
        S: ?Sized + Debug + for<'a> Into<Cow<'a, str>> + Send,
    {
        let (mut state, mut client) = self.state_client().await?;
        let mut results = client.query(statement, params).await.map_err(Error::std)?;
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

        state.client = Some(client);

        Ok(coll)
    }
}

pub struct MssqlTransaction<'a, F> {
    id: u64,
    provider: &'a MssqlProvider<F>,
}

impl<'a, F> MssqlTransaction<'a, F> {
    #[instrument(name = "MssqlTransaction::commit", skip(self), err)]
    pub async fn commit(mut self) -> Result<()> {
        self.id = 0;

        let (mut state, mut client) = self.state_client().await?;

        state.current_transaction = 0;
        client.simple_query("COMMIT").await.map_err(Error::std)?;
        state.client = Some(client);

        Ok(())
    }

    async fn state_client(&self) -> Result<(MutexGuard<'_, State>, Client)> {
        let mut state = self.provider.state.lock().await;

        if state.current_transaction != self.id {
            return Err(Error::NotInTransaction);
        }

        if let Some(client) = state.client.take() {
            return Ok((state, client));
        }

        state.current_transaction = 0;
        Err(Error::NotInTransaction)
    }
}

impl<'a, F> Drop for MssqlTransaction<'a, F> {
    fn drop(&mut self) {
        if self.id > 0 {
            self.provider.cancel_transaction.store(self.id, Relaxed);
        }
    }
}

#[async_trait]
impl<'a, F> Execute for MssqlTransaction<'a, F>
where
    F: Send + Sync,
{
    #[instrument(name = "MssqlTransaction::execute", skip(self, params), err)]
    async fn execute<'b, S>(&self, statement: S, params: &[&(dyn ToSql)]) -> Result<u64>
    where
        S: ?Sized + Debug + Into<Cow<'b, str>> + Send,
    {
        let (mut state, mut client) = self.state_client().await?;

        let count = client
            .execute(statement, params)
            .await
            .map_err(Error::std)?
            .total();

        state.client = Some(client);
        Ok(count)
    }
}

#[async_trait]
impl<'a, 'b, F> Gate<'b> for MssqlTransaction<'a, F>
where
    F: Send + Sync,
{
    type Gate = MutexGuard<'b, ()>;

    async fn gate(&'b self) -> Self::Gate {
        self.provider.gate.lock().await
    }
}

#[async_trait]
impl<'a, F> QueryRows for MssqlTransaction<'a, F>
where
    F: ClientFactory,
{
    async fn query_rows<S, M, R, C>(
        &self,
        statement: S,
        params: &[&(dyn ToSql)],
        mapper: M,
    ) -> Result<C>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send,
        R: Send,
        S: ?Sized + Debug + for<'b> Into<Cow<'b, str>> + Send,
    {
        self.provider.query_rows(statement, params, mapper).await
    }
}

#[async_trait::async_trait]
impl<'a, E, F> storm::provider::LoadAll<E> for MssqlTransaction<'a, F>
where
    E: storm::Entity + Send + 'a,
    E::Key: Send,
    F: ClientFactory,
    MssqlProvider<F>: storm::provider::LoadAll<E>,
{
    async fn load_all<C: Default + Extend<(<E as storm::Entity>::Key, E)> + Send>(
        &self,
    ) -> storm::Result<C> {
        storm::provider::LoadAll::<E>::load_all(self.provider).await
    }
}

struct State {
    client: Option<Client>,
    transaction_counter: u64,
    current_transaction: u64,
}
