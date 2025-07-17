use crate::{BoxFuture, Result};
use parking_lot::Mutex;

pub trait Init<'a, P>: Sized + Send + Sync {
    fn init(provider: &'a P) -> BoxFuture<'a, Result<Self>>;
}

type Func<Tbl> = Box<dyn FnMut(&mut Tbl) + Send + Sync>;

pub struct Inits<Tbl>(Mutex<Vec<Func<Tbl>>>);

impl<Tbl> Inits<Tbl> {
    #[doc(hidden)]
    pub const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    pub(crate) fn apply(&self, tbl: &mut Tbl) {
        self.0.lock().iter_mut().for_each(|f| f(tbl));
    }

    /// register function to be called at initialization.
    pub fn register<F>(&self, init: F)
    where
        F: FnMut(&mut Tbl) + Send + Sync + 'static,
    {
        self.0.lock().push(Box::new(init));
    }
}
