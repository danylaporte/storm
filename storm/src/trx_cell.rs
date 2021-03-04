use crate::{mem, GetOrLoadAsync, Result};
use once_cell::sync::OnceCell;

pub struct TrxCell<'a, T: mem::Transaction<'a>> {
    ctx: &'a OnceCell<T>,
    trx: OnceCell<T::Transaction>,
}

impl<'a, T: mem::Transaction<'a>> mem::Commit for TrxCell<'a, T>
where
    T::Transaction: mem::Commit,
{
    type Log = Option<<T::Transaction as mem::Commit>::Log>;

    fn commit(self) -> Self::Log {
        self.trx.into_inner().map(|t| t.commit())
    }
}

impl<'a, T: mem::Transaction<'a>> TrxCell<'a, T> {
    pub fn new(ctx: &'a OnceCell<T>) -> Self {
        Self {
            ctx,
            trx: OnceCell::new(),
        }
    }

    pub fn get_mut<'b>(&'b mut self) -> Option<&'b mut <T as mem::Transaction<'a>>::Transaction>
    where
        T: mem::Transaction<'a>,
    {
        self.trx.get_mut()
    }

    pub async fn get_mut_or_init<'b, P>(
        &'b mut self,
        provider: &P,
    ) -> Result<&'b mut <T as mem::Transaction<'a>>::Transaction>
    where
        OnceCell<T>: GetOrLoadAsync<T, P>,
        T: mem::Transaction<'a>,
    {
        if self.trx.get().is_none() {
            let ctx = GetOrLoadAsync::get_or_load_async(self.ctx, provider).await?;
            let trx = ctx.transaction();
            let _ = self.trx.set(trx);
        }

        Ok(self.trx.get_mut().expect("TrxCell"))
    }

    pub async fn get_or_init<'b, P>(
        &'b self,
        provider: &P,
    ) -> Result<&'b <T as mem::Transaction<'a>>::Transaction>
    where
        OnceCell<T>: GetOrLoadAsync<T, P>,
        T: mem::Transaction<'a>,
    {
        if let Some(v) = self.trx.get() {
            return Ok(v);
        }

        let ctx = GetOrLoadAsync::get_or_load_async(self.ctx, provider).await?;
        Ok(self.trx.get_or_init(|| ctx.transaction()))
    }
}
