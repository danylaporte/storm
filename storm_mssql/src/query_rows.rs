use async_trait::async_trait;
use std::{borrow::Cow, fmt::Debug};
use storm::Result;
use tiberius::{Row, ToSql};

#[async_trait]
pub trait QueryRows {
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
        S: ?Sized + Debug + for<'a> Into<Cow<'a, str>> + Send;
}
