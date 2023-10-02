use crate::Entity;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub enum Changed<E> {
    Inserted { old: Option<E>, new: E },
    Removed { old: E },
}

impl<E> Changed<E> {
    pub fn new_old(&self) -> (Option<&E>, Option<&E>) {
        match self {
            Self::Inserted { old, new } => (Some(new), old.as_ref()),
            Self::Removed { old } => (None, Some(old)),
        }
    }
}

pub trait ChangedHandler<E: Entity> {
    fn handle_changed(&self, key: &E::Key, entity: Changed<&E>);
}

impl<E: Entity, T> ChangedHandler<E> for T
where
    T: Fn(&E::Key, Changed<&E>),
{
    fn handle_changed(&self, key: &<E as Entity>::Key, entity: Changed<&E>) {
        (self)(key, entity)
    }
}

type ArcChangedHandler<E> = Arc<dyn ChangedHandler<E> + Send + Sync>;

pub struct OnChanged<E>(parking_lot::Mutex<Arc<Box<[ArcChangedHandler<E>]>>>);

impl<E: Entity> OnChanged<E> {
    #[doc(hidden)]
    pub fn __call(&self, key: &E::Key, entity: Changed<&E>) {
        let vec = Arc::clone(&self.0.lock());

        for handler in vec.iter() {
            handler.handle_changed(key, entity);
        }
    }

    pub fn register<H: ChangedHandler<E> + Send + Sync + 'static>(&self, handler: H) {
        self.register_impl(Arc::new(handler));
    }

    pub fn register_fn<F>(&self, f: F)
    where
        F: Fn(&E::Key, Changed<&E>) + Send + Sync + 'static,
    {
        self.register(f);
    }

    fn register_impl(&self, handler: ArcChangedHandler<E>) {
        let mut gate = self.0.lock();
        let mut vec = Vec::with_capacity(gate.len() + 1);

        vec.extend(gate.iter().cloned());
        vec.push(handler);

        *gate = Arc::new(vec.into_boxed_slice());
    }
}

impl<E> Default for OnChanged<E> {
    fn default() -> Self {
        Self(Default::default())
    }
}
