use crate::{Gc, LogToken, ObjBase, Trx};
use std::ops::{Deref, DerefMut};

pub type Log<T> = Option<T>;

/// Represent a Copy on write object that support a transaction.
pub struct CowObj<T>(pub T);

impl<T: Gc> Gc for CowObj<T> {
    const SUPPORT_GC: bool = T::SUPPORT_GC;

    #[inline]
    fn gc(&mut self) {
        self.0.gc();
    }
}

impl<T> ObjBase for CowObj<T>
where
    T: PartialEq + Send + Sync + 'static,
{
    type Log = Log<T>;
    type Trx<'a> = CowTrx<'a, T>;

    fn apply_log(&mut self, log: Self::Log) -> bool {
        if let Some(log) = log {
            if self.0 != log {
                self.0 = log;
                return true;
            }
        }

        false
    }

    fn trx<'a>(&'a self, trx: &'a mut Trx<'a>, log: LogToken<Self::Log>) -> Self::Trx<'a> {
        CowTrx {
            obj: self,
            log_token: log,
            trx,
        }
    }
}

impl<T> Deref for CowObj<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct CowTrx<'a, T> {
    obj: &'a T,
    log_token: LogToken<Log<T>>,
    trx: &'a mut Trx<'a>,
}

impl<'a, T> Deref for CowTrx<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.trx
            .log
            .get(&self.log_token)
            .and_then(|o| o.as_ref())
            .unwrap_or(self.obj)
    }
}

impl<'a, T> DerefMut for CowTrx<'a, T>
where
    T: Clone,
{
    #[allow(clippy::unwrap_used)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        let log = self.trx.log.get_or_init_mut(&self.log_token);

        match log {
            Some(log) => log,
            None => {
                *log = Some(self.obj.clone());
                log.as_mut().unwrap()
            }
        }
    }
}
