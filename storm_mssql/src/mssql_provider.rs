use crate::{Client, ClientFactory, Execute, FilterSql, QueryRows};
use async_trait::async_trait;
use futures_util::TryStreamExt;
use std::{
    borrow::Cow,
    fmt::Debug,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};
use storm::{provider, Error, Result};
use tiberius::{Row, ToSql};
use tokio::sync::{Mutex, MutexGuard};
use tracing::instrument;

pub struct MssqlProvider {
    cancel_transaction: AtomicU64,
    client_factory: Box<dyn ClientFactory>,
    state: Mutex<State>,
}

impl From<Box<dyn ClientFactory>> for MssqlProvider {
    fn from(client_factory: Box<dyn ClientFactory>) -> Self {
        Self {
            cancel_transaction: Default::default(),
            client_factory,
            state: Mutex::new(State {
                client: None,
                current_transaction: 0,
                transaction_counter: 1,
            }),
        }
    }
}

impl MssqlProvider {
    pub fn new<F: ClientFactory + 'static>(client_factory: F) -> Self {
        (Box::new(client_factory) as Box<dyn ClientFactory>).into()
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
    pub async fn transaction(&self) -> Result<MssqlTransaction<'_>> {
        let (mut state, mut client) = self.state_client().await?;

        if state.current_transaction > 0 {
            state.client = Some(client);
            return Err(Error::AlreadyInTransaction);
        }

        client
            .simple_query("BEGIN TRAN")
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
impl QueryRows for MssqlProvider {
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

#[async_trait]
impl<'a> provider::Transaction<'a> for MssqlProvider {
    type Transaction = MssqlTransaction<'a>;

    async fn transaction(&'a self) -> Result<Self::Transaction> {
        MssqlProvider::transaction(self).await
    }
}

pub struct MssqlTransaction<'a> {
    id: u64,
    provider: &'a MssqlProvider,
}

impl<'a> MssqlTransaction<'a> {
    #[instrument(name = "MssqlTransaction::commit", skip(self), err)]
    pub async fn commit(mut self) -> Result<()> {
        let result = transaction_state_client(self.provider, self.id).await;

        // indicate that the transaction is now disposed.
        self.id = 0;

        let (mut state, mut client) = result?;

        state.current_transaction = 0;
        client.simple_query("COMMIT").await.map_err(Error::std)?;
        state.client = Some(client);

        Ok(())
    }
}

#[async_trait]
impl<'a> provider::Commit for MssqlTransaction<'a> {
    async fn commit(self) -> Result<()> {
        MssqlTransaction::commit(self).await
    }
}

impl<'a> Drop for MssqlTransaction<'a> {
    fn drop(&mut self) {
        if self.id > 0 {
            self.provider.cancel_transaction.store(self.id, Relaxed);
        }
    }
}

#[async_trait]
impl<'a> Execute for MssqlTransaction<'a> {
    #[instrument(name = "MssqlTransaction::execute", skip(self, params), err)]
    async fn execute<'b, S>(&self, statement: S, params: &[&(dyn ToSql)]) -> Result<u64>
    where
        S: ?Sized + Debug + Into<Cow<'b, str>> + Send,
    {
        let (mut state, mut client) = transaction_state_client(self.provider, self.id).await?;

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
impl<'a> QueryRows for MssqlTransaction<'a> {
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

#[async_trait]
impl<'a, C, E, FILTER> storm::provider::LoadAll<E, FILTER, C> for MssqlTransaction<'a>
where
    C: Default + Extend<(E::Key, E)> + Send + 'static,
    E: storm::Entity + Send + 'a,
    E::Key: Send,
    FILTER: FilterSql,
    MssqlProvider: storm::provider::LoadAll<E, FILTER, C>,
{
    async fn load_all(&self, filter: &FILTER) -> storm::Result<C> {
        storm::provider::LoadAll::<E, FILTER, C>::load_all(self.provider, filter).await
    }
}

struct State {
    client: Option<Client>,
    transaction_counter: u64,
    current_transaction: u64,
}

async fn transaction_state_client(
    provider: &MssqlProvider,
    transaction_id: u64,
) -> Result<(MutexGuard<'_, State>, Client)> {
    let mut state = provider.state.lock().await;

    if state.current_transaction != transaction_id {
        return Err(Error::NotInTransaction);
    }

    if let Some(client) = state.client.take() {
        return Ok((state, client));
    }

    state.current_transaction = 0;
    Err(Error::NotInTransaction)
}
