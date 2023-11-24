use crate::ToSql;
use storm::{BoxFuture, Result};

pub trait Execute {
    fn execute_with_args<'a>(
        &'a self,
        statement: String,
        params: &'a [&'a (dyn ToSql)],
        args: ExecuteArgs,
    ) -> BoxFuture<'a, Result<u64>>;

    #[inline]
    fn execute<'a>(
        &'a self,
        statement: String,
        params: &'a [&'a (dyn ToSql)],
    ) -> BoxFuture<'a, Result<u64>> {
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
