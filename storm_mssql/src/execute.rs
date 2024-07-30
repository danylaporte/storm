use crate::ToSql;
use std::{borrow::Cow, fmt::Debug};
use storm::{BoxFuture, Result};

pub trait Execute {
    fn execute_with_args<'a, S>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
        args: ExecuteArgs,
    ) -> BoxFuture<'a, Result<u64>>
    where
        S: Debug + Into<Cow<'a, str>> + Send + 'a;

    #[inline]
    fn execute<'a, S>(
        &'a self,
        statement: S,
        params: &'a [&'a (dyn ToSql)],
    ) -> BoxFuture<'a, Result<u64>>
    where
        S: Debug + Into<Cow<'a, str>> + Send + 'a,
    {
        self.execute_with_args(statement, params, ExecuteArgs::default())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExecuteArgs {
    pub use_transaction: bool,
}

impl Default for ExecuteArgs {
    fn default() -> Self {
        Self {
            use_transaction: true,
        }
    }
}
