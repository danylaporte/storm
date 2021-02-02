use crate::{ClientFactory, Query};
use async_trait::async_trait;
use std::{fmt::Debug, intrinsics::transmute};
use storm::{Error, OptsTransaction, Result};
use tokio::sync::Mutex;
use tokio_postgres::{types::ToSql, Client, Row, ToStatement, Transaction};
use tracing::{error, instrument};

pub struct ConnPool<C> {
    client_factory: C,
    state: Mutex<State>,
}

#[async_trait]
impl<C> OptsTransaction for ConnPool<C>
where
    C: ClientFactory + Send + Sync,
{
    async fn cancel(&self) {
        let mut state = self.state.lock().await;

        if state.transaction.take().is_none() {
            error!("Attempt to cancel a non existing transaction.");
        }
    }

    #[instrument(name = "ConnPool::commit", skip(self), err)]
    async fn commit(&self) -> storm::Result<()> {
        let mut state = self.state.lock().await;

        match state.transaction.take() {
            Some(t) => t.commit().await.map_err(Error::std),
            None => Err(Error::NotInTransaction),
        }
    }

    #[instrument(name = "ConnPool::transaction", skip(self), err)]
    async fn transaction(&self) -> storm::Result<()> {
        let mut state = self.state.lock().await;

        if state.transaction.is_some() {
            return Err(Error::AlreadyInTransaction);
        }

        state.ensure_client(&self.client_factory).await?;

        let client = state.client.as_mut().unwrap();
        let transaction: Transaction = client.transaction().await.map_err(Error::std)?;
        let transaction: Transaction<'static> = unsafe { transmute(transaction) };

        state.transaction = Some(transaction);

        Ok(())
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
        S: Debug + ToStatement + Send + Sync,
    {
        let mut state = self.state.lock().await;

        if let Some(t) = state.transaction.as_ref() {
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
}
