use crate::{Client, ClientFactory, Execute, Query};
use async_trait::async_trait;
use std::{
    borrow::Cow,
    fmt::Debug,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};
use storm::{Error, OptsTransaction, OptsVersion, Result};
use tiberius::{Row, ToSql};
use tokio::sync::Mutex;
use tracing::{error, instrument};
use transaction_states::{ACTIVE, CANCELLED, NONE};

pub struct ConnPool<C> {
    client_factory: C,
    state: Mutex<State>,
    transaction_state: AtomicUsize,
    version: u64,
}

#[async_trait]
impl<C> Execute for ConnPool<C>
where
    C: ClientFactory + Send + Sync,
{
    #[instrument(skip(self, params), err)]
    async fn execute<'a, S>(&self, statement: S, params: &[&(dyn ToSql)]) -> Result<u64>
    where
        S: ?Sized + Debug + Into<Cow<'a, str>> + Send,
    {
        let mut state = self.state.lock().await;

        if self.transaction_state.load(Relaxed) != ACTIVE {
            return Err(Error::NotInTransaction);
        }

        match state.client.take() {
            Some(mut client) => {
                let count = client
                    .execute(statement, params)
                    .await
                    .map_err(Error::std)?
                    .total();

                state.client = Some(client);
                Ok(count)
            }
            None => Err(Error::NotInTransaction),
        }
    }
}

#[async_trait]
impl<C> OptsTransaction for ConnPool<C>
where
    C: ClientFactory + Send + Sync,
{
    fn cancel(&self) {
        if self
            .transaction_state
            .compare_and_swap(ACTIVE, CANCELLED, Relaxed)
            != ACTIVE
        {
            error!("Attempt to cancel a non existing transaction.");
        }
    }

    #[instrument(name = "ConnPool::commit", skip(self), err)]
    async fn commit(&self) -> storm::Result<()> {
        let mut state = self.state.lock().await;

        state
            .clean_cancelled_transaction(&self.transaction_state)
            .await?;

        if self
            .transaction_state
            .compare_and_swap(ACTIVE, NONE, Relaxed)
            != ACTIVE
        {
            return Err(Error::NotInTransaction);
        }

        if let Some(mut client) = state.client.take() {
            client.simple_query("COMMIT").await.map_err(Error::std)?;
            state.client = Some(client);
        }

        Ok(())
    }

    #[instrument(name = "ConnPool::transaction", skip(self), err)]
    async fn transaction(&self) -> storm::Result<()> {
        let mut state = self.state.lock().await;

        if self.transaction_state.load(Relaxed) == ACTIVE {
            return Err(Error::AlreadyInTransaction);
        }

        state
            .clean_cancelled_transaction(&self.transaction_state)
            .await?;

        let mut client = state.take_or_init_client(&self.client_factory).await?;
        client
            .simple_query("BEGIN TRANS")
            .await
            .map_err(Error::std)?;

        self.transaction_state.store(ACTIVE, Relaxed);
        state.client = Some(client);

        Ok(())
    }
}

impl<C> OptsVersion for ConnPool<C> {
    fn opts_new_version(&mut self) -> u64 {
        self.version += 1;
        self.version
    }

    fn opts_version(&self) -> u64 {
        self.version
    }
}

#[async_trait]
impl<C> Query for ConnPool<C>
where
    C: ClientFactory + Send + Sync,
{
    #[instrument(skip(self, params), err)]
    async fn query_rows<S>(&self, statement: S, params: &[&(dyn ToSql)]) -> Result<Vec<Row>>
    where
        S: ?Sized + Debug + for<'a> Into<Cow<'a, str>> + Send,
    {
        let mut state = self.state.lock().await;

        if self.transaction_state.load(Relaxed) != ACTIVE {
            return Err(Error::NotInTransaction);
        }

        match state.client.take() {
            Some(mut client) => {
                let vec = client
                    .query(statement, params)
                    .await
                    .map_err(Error::std)?
                    .into_first_result()
                    .await
                    .map_err(Error::std)?;

                state.client = Some(client);
                Ok(vec)
            }
            None => Err(Error::NotInTransaction),
        }
    }
}

struct State {
    client: Option<Client>,
}

impl State {
    async fn clean_cancelled_transaction(&mut self, transaction_state: &AtomicUsize) -> Result<()> {
        if transaction_state.compare_and_swap(CANCELLED, NONE, Relaxed) == CANCELLED {
            if let Some(mut client) = self.client.take() {
                client.simple_query("ROLLBACK").await.map_err(Error::std)?;
                self.client = Some(client);
            }
        }

        Ok(())
    }

    async fn take_or_init_client<C>(&mut self, client_factory: &C) -> Result<Client>
    where
        C: ClientFactory,
    {
        match self.client.take() {
            Some(client) => Ok(client),
            None => client_factory.create_client().await,
        }
    }
}

mod transaction_states {
    pub const NONE: usize = 0;
    pub const ACTIVE: usize = 1;
    pub const CANCELLED: usize = 2;
}
