use crate::ToSql;
use std::{borrow::Cow, fmt::Debug};
use storm::{BoxFuture, Result};
use tiberius::Row;

pub trait QueryRows {
    /// Execute a query on the sql server and returns the row.
    ///
    /// ## Parameters
    /// - use_transaction: make sure the query is run inside a transaction.
    ///
    /// This is useful when loading we need to execute a query and then load the result
    /// from sql from the same transaction.
    fn query_rows<'a, S, M, R, C>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
        mapper: M,
        use_transaction: bool,
    ) -> BoxFuture<'a, Result<C>>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send + 'a,
        R: Send,
        S: Debug + for<'b> Into<Cow<'b, str>> + Send + 'a;
}

impl<P> QueryRows for &P
where
    P: QueryRows + Send + Sync,
{
    fn query_rows<'a, S, M, R, C>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
        mapper: M,
        use_transaction: bool,
    ) -> BoxFuture<'a, Result<C>>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send + 'a,
        R: Send,
        S: Debug + for<'b> Into<Cow<'b, str>> + Send + 'a,
    {
        (**self).query_rows(statement, params, mapper, use_transaction)
    }
}
