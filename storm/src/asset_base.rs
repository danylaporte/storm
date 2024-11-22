use crate::{log::LogToken, Trx};

/// A table or an index in the storm Ctx is an asset.
pub trait AssetBase: Sized + Send + Sync + 'static {
    const SUPPORT_GC: bool = false;

    type Log: Default;
    type Trx<'a: 'b, 'b>;

    fn apply_log(&mut self, log: Self::Log) -> bool;

    fn gc(&mut self);

    fn trx<'a: 'b, 'b>(
        &'b self,
        trx: &'b mut Trx<'a>,
        log: LogToken<Self::Log>,
    ) -> Self::Trx<'a, 'b>;
}
