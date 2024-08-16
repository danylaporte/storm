use crate::{Error, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering::Relaxed},
    Arc,
};

pub(crate) struct TrxErrGate(Option<Arc<AtomicBool>>);

impl TrxErrGate {
    pub fn check(&self) -> Result<()> {
        match self.get_inner() {
            Some(_) => Ok(()),
            None => Err(Error::TransactionError),
        }
    }

    pub fn close(mut self) {
        self.0 = None;
    }

    pub fn open(&self) -> Result<Self> {
        if let Some(a) = self.get_inner() {
            Ok(Self(Some(Arc::clone(a))))
        } else {
            Err(Error::TransactionError)
        }
    }

    fn get_inner(&self) -> Option<&Arc<AtomicBool>> {
        self.0.as_ref().filter(|a| !a.load(Relaxed))
    }
}

impl Default for TrxErrGate {
    fn default() -> Self {
        Self(Some(Arc::new(AtomicBool::new(false))))
    }
}

impl Drop for TrxErrGate {
    fn drop(&mut self) {
        if let Some(v) = self.0.take() {
            v.store(true, Relaxed);
        }
    }
}
