use crate::{Ctx, CtxExt, Logs, registry::InitCell};
use extobj::VarId;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::cmp::Ordering;

pub trait ApplyLog<L> {
    fn apply_log(&mut self, log: L) -> bool;
}

type ApplierFn = fn(ctx: &mut Ctx, logs: &mut Logs) -> bool;

struct Applier {
    f: ApplierFn,
    order: ApplyOrder,
    triggers: Box<[VarId<CtxExt>]>,
}

struct Appliers {
    /// Sorted by `ApplyOrder`.
    list: Vec<Applier>,

    /// For each log var, the ascending positions in `list` of the appliers it triggers.
    by_var: FxHashMap<VarId<CtxExt>, Box<[u32]>>,
}

static APPLIERS: InitCell<Appliers> = InitCell::new(Appliers {
    list: Vec::new(),
    by_var: FxHashMap::with_hasher(FxBuildHasher),
});

/// Private: use by a macro.
///
/// `triggers` are the log vars consumed by the applier; it is only invoked when at
/// least one of them is present in the logs being applied.
#[doc(hidden)]
pub fn __register_apply(f: ApplierFn, order: ApplyOrder, triggers: &[VarId<CtxExt>]) {
    let list = &mut APPLIERS.get_mut().list;

    let (Ok(index) | Err(index)) = list.binary_search_by_key(&order, |a| a.order);

    list.insert(
        index,
        Applier {
            f,
            order,
            triggers: triggers.into(),
        },
    );
}

/// Builds the var → appliers lookup. Must be called at the end of the registration phase.
pub(crate) fn finalize_appliers() {
    let appliers = APPLIERS.get_mut();
    let mut by_var = FxHashMap::<VarId<CtxExt>, Vec<u32>>::default();

    for (index, applier) in appliers.list.iter().enumerate() {
        let index = u32::try_from(index).expect("too many appliers");

        for var in &applier.triggers {
            let positions = by_var.entry(*var).or_default();

            if positions.last() != Some(&index) {
                positions.push(index);
            }
        }
    }

    appliers.by_var = by_var.into_iter().map(|(k, v)| (k, v.into())).collect();
}

pub(crate) fn perform_apply_log(ctx: &mut Ctx, mut logs: Logs) -> bool {
    let appliers = APPLIERS.get();
    let triggered = |var| appliers.by_var.get(&var).map_or(&[][..], |p| &p[..]);
    let mut changed = false;

    match logs.len() {
        0 => {}
        1 => {
            let var = logs.var_ids().next().expect("one log");

            for &index in triggered(var) {
                changed |= (appliers.list[index as usize].f)(ctx, &mut logs);
            }
        }
        _ => {
            // Merge the triggered appliers of every log, keeping the global apply order.
            let mut positions = logs
                .var_ids()
                .flat_map(triggered)
                .copied()
                .collect::<Vec<_>>();

            positions.sort_unstable();
            positions.dedup();

            for index in positions {
                changed |= (appliers.list[index as usize].f)(ctx, &mut logs);
            }
        }
    }

    changed
}

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
