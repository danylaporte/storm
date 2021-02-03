use crate::{ClientFactory, Execute, Query};
use async_trait::async_trait;
use postgres_types::ToSql;
use std::{
    fmt::Debug,
    intrinsics::transmute,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};
use storm::{Error, OptsTransaction, OptsVersion, Result};
use tokio::sync::Mutex;
use tokio_postgres::{Client, Row, ToStatement, Transaction};
use tracing::{error, instrument};

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
    #[instrument(skip(self), err)]
    async fn execute<S>(&self, statement: &S, params: &[&(dyn ToSql + Sync)]) -> Result<u64>
    where
        S: ?Sized + Debug + ToStatement + Send + Sync,
    {
        let mut state = self.state.lock().await;

        match state.transaction(&self.transaction_state) {
            Some(t) => t.execute(statement, params).await.map_err(Error::std),
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
        use transaction_states::{ACTIVE, CANCELLED};

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

        // update transaction status.
        state.transaction(&self.transaction_state);
        self.transaction_state.store(0, Relaxed);

        match state.transaction.take() {
            Some(t) => t.commit().await.map_err(Error::std),
            None => Err(Error::NotInTransaction),
        }
    }

    #[instrument(name = "ConnPool::transaction", skip(self), err)]
    async fn transaction(&self) -> storm::Result<()> {
        let mut state = self.state.lock().await;

        if state.transaction(&self.transaction_state).is_some() {
            return Err(Error::AlreadyInTransaction);
        }

        state.ensure_client(&self.client_factory).await?;

        let client = state.client.as_mut().unwrap();
        let transaction: Transaction = client.transaction().await.map_err(Error::std)?;
        let transaction: Transaction<'static> = unsafe { transmute(transaction) };

        state.transaction = Some(transaction);

        self.transaction_state
            .store(transaction_states::ACTIVE, Relaxed);

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
    #[instrument(skip(self), err)]
    async fn query_rows<S, P>(
        &self,
        statement: &S,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>>
    where
        S: ?Sized + Debug + ToStatement + Send + Sync,
    {
        let mut state = self.state.lock().await;

        if let Some(t) = state.transaction(&self.transaction_state) {
            return t.query(statement, params).await.map_err(Error::std);
        }

        state
            .ensure_client(&self.client_factory)
            .await?
            .query(statement, params)
            .await
            .map_err(Error::std)
    }
}

struct State {
    client: Option<Client>,
    transaction: Option<Transaction<'static>>,
}

impl State {
    async fn ensure_client<C>(&mut self, client_factory: &C) -> Result<&Client>
    where
        C: ClientFactory,
    {
        if self.client.is_none() {
            self.client = Some(client_factory.create_client().await?);
        }

        Ok(self.client.as_ref().unwrap())
    }

    fn transaction(&mut self, transaction_state: &AtomicUsize) -> Option<&Transaction<'_>> {
        use transaction_states::{CANCELLED, NONE};

        if transaction_state.compare_and_swap(CANCELLED, NONE, Relaxed) == CANCELLED {
            self.transaction = None;
        }

        self.transaction.as_ref()
    }
}

mod transaction_states {
    pub const NONE: usize = 0;
    pub const ACTIVE: usize = 1;
    pub const CANCELLED: usize = 2;
}
