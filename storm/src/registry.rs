use chrono::NaiveDateTime;
use std::cell::{Cell, UnsafeCell};

pub(crate) struct InitCell<T>(UnsafeCell<T>);

impl<T> InitCell<T> {
    #[inline]
    pub(crate) const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }

    #[inline]
    pub fn get(&self) -> &T {
        unsafe { &*self.0.get() }
    }

    #[allow(clippy::mut_from_ref)]
    #[inline]
    pub fn get_mut(&self) -> &mut T {
        check_can_register();
        unsafe { &mut *self.0.get() }
    }
}

unsafe impl<T: Sync> Send for InitCell<T> {}
unsafe impl<T: Sync> Sync for InitCell<T> {}

/// Private : For macro only.
#[doc(hidden)]
#[linkme::distributed_slice]
pub static __REGISTRATION: [fn()];

pub(crate) fn check_can_register() {
    assert!(
        IN_REGISTRATION.get(),
        "must be call inside the registration phase."
    );
}

pub(crate) fn perform_registration() {
    static O: parking_lot::Once = parking_lot::Once::new();

    O.call_once(|| {
        IN_REGISTRATION.set(true);

        for f in __REGISTRATION {
            f();
        }

        IN_REGISTRATION.set(false);
    });
}

thread_local! {
    static IN_REGISTRATION: Cell<bool> = const { Cell::new(false) };
}

fn default_date_provider() -> NaiveDateTime {
    chrono::Local::now().naive_local()
}

static DATE_PROVIDER: InitCell<fn() -> NaiveDateTime> = InitCell::new(default_date_provider);

pub(crate) fn provide_date() -> NaiveDateTime {
    (DATE_PROVIDER.get())()
}

pub fn set_date_provider(provider: fn() -> NaiveDateTime) {
    *DATE_PROVIDER.get_mut() = provider;
}
