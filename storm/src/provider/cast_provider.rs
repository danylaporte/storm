use std::any::Any;

use super::Provider;

pub(super) struct CastProvider {
    downcast: *const (dyn Any + Send + Sync),
    provider: Box<dyn Provider>,
}

impl CastProvider {
    pub fn new(p: impl Provider) -> Self {
        let provider = Box::new(p);

        Self {
            downcast: &*provider,
            provider,
        }
    }

    pub fn downcast<T: 'static>(&self) -> Option<&T> {
        unsafe { &*self.downcast }.downcast_ref()
    }

    pub fn provider(&self) -> &dyn Provider {
        self.provider.as_ref()
    }
}

unsafe impl Send for CastProvider {}
unsafe impl Sync for CastProvider {}
