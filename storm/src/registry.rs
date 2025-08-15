use chrono::NaiveDateTime;
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

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

static REGISTRATION_DONE: AtomicBool = AtomicBool::new(false);

pub(crate) fn check_can_register() {
    assert!(
        !REGISTRATION_DONE.load(Ordering::Relaxed),
        "storm register phase is closed."
    );
}

pub(crate) fn perform_registration() {
    static O: std::sync::Once = std::sync::Once::new();

    O.call_once(|| {
        for f in __REGISTRATION {
            f();
        }
        REGISTRATION_DONE.store(true, Ordering::Relaxed);
    });
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
