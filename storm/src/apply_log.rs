use crate::{registry::InitCell, Ctx, Logs};
use std::cmp::Ordering;

pub trait ApplyLog<L> {
    fn apply_log(&mut self, log: L) -> bool;
}

type ApplierFn = fn(ctx: &mut Ctx, logs: &mut Logs) -> bool;

/// Private: use by a macro.
#[doc(hidden)]
pub fn __register_apply(f: ApplierFn, order: ApplyOrder) {
    let vec = APPLIERS.get_mut();
    match vec.binary_search_by_key(&order, |t| t.1) {
        Ok(index) | Err(index) => vec.insert(index, (f, order)),
    }
}

#[inline]
pub(crate) fn perform_apply_log(ctx: &mut Ctx, mut logs: Logs) -> bool {
    let mut changed = false;

    for (f, _) in APPLIERS.get() {
        changed |= f(ctx, &mut logs);
    }

    changed
}

static APPLIERS: InitCell<Vec<(ApplierFn, ApplyOrder)>> = InitCell::new(Vec::new());

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum ApplyOrder {
    FlatSet = 5,
    NodeSet = 10,
    Tree = 15,
    Table = 20,
}

impl Eq for ApplyOrder {}

impl Ord for ApplyOrder {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl PartialOrd for ApplyOrder {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ApplyOrder {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        (*self as u8) == (*other as u8)
    }
}
