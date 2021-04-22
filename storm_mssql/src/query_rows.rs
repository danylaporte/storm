use crate::ToSql;
use std::{borrow::Cow, fmt::Debug};
use storm::{BoxFuture, Result};
use tiberius::Row;

pub trait QueryRows {
    fn query_rows<'a, S, M, R, C>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
        mapper: M,
    ) -> BoxFuture<'a, Result<C>>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send + 'a,
        R: Send,
        S: ?Sized + Debug + for<'b> Into<Cow<'b, str>> + Send + 'a;
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
    ) -> BoxFuture<'a, Result<C>>
    where
        C: Default + Extend<R> + Send,
        M: FnMut(Row) -> Result<R> + Send + 'a,
        R: Send,
        S: ?Sized + Debug + for<'b> Into<Cow<'b, str>> + Send + 'a,
    {
        (**self).query_rows(statement, params, mapper)
    }
}
