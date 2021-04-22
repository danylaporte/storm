use crate::ToSql;
use std::{borrow::Cow, fmt::Debug};
use storm::{BoxFuture, Result};

pub trait Execute {
    fn execute<'a, S>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
    ) -> BoxFuture<'a, Result<u64>>
    where
        S: ?Sized + Debug + Into<Cow<'a, str>> + Send + 'a;
}
