//! Dry-run of a cascaded removal, executable in batches.
//!
//! [DeletePlan] walks the foreign keys (`on_delete = "cascade" | "set_null" | "restrict"`)
//! and the business rules ([on_remove_rule](crate::on_remove_rule)) from a root entity without modifying anything. The plan can be inspected (graph, counts,
//! restrictions) and then executed leaves first, in multiple transactions, so the queue lock
//! can be released between batches and the progress reported.
//!
//! Every step is idempotent and re-validated when executed: a step is skipped when its entity
//! is gone or when any link of its foreign key chain up to the root no longer holds. Rows
//! created after the plan are still cascaded when their parent is removed.

use crate::{
    AsRefAsync, BoxFuture, Ctx, CtxTransaction, CtxTypeInfo, Entity, EntityAccessor, EntityRemove,
    Error, ProviderContainer, Result,
    accessor::entity_of,
    error::debug_string,
    foreign_key::{
        ChildKey, ForeignKey, ForeignKeySetNull, ParentKey, children_of, count_children,
    },
    provider::{Delete, LoadAll, TransactionProvider},
    registry::InitCell,
    remove_rule,
};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::{
    any::{Any, TypeId},
    fmt::Debug,
    marker::PhantomData,
};

type PlanFn = for<'a> fn(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a (dyn Any + Send + Sync),
    planner: &'a mut DeletePlanner,
) -> BoxFuture<'a, Result<()>>;

/// Planning handlers by entity type, in registration order.
static PLANNERS: InitCell<FxHashMap<TypeId, Vec<PlanFn>>> =
    InitCell::new(FxHashMap::with_hasher(FxBuildHasher));

/// Registers a planning handler called when an entity `P` is planned for removal.
pub fn on_planning<P: 'static>(f: PlanFn) {
    PLANNERS
        .get_mut()
        .entry(TypeId::of::<P>())
        .or_default()
        .push(f);
}

#[inline]
fn planners_of(ty: TypeId) -> &'static [PlanFn] {
    PLANNERS.get().get(&ty).map_or(&[], Vec::as_slice)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeleteAction {
    Remove,
    SetNull,
}

/// A restriction blocking the plan.
#[derive(Clone, Debug)]
pub enum DeleteRestriction {
    /// A foreign key with `on_delete = "restrict"`.
    ForeignKey {
        /// Index of the blocked node in the plan.
        node: usize,
        entity: &'static str,
        field: &'static str,
        parent: &'static str,
        key: String,
        count: usize,
    },

    /// A business rule registered with [on_remove_rule](crate::on_remove_rule).
    Rule {
        /// Index of the blocked node in the plan.
        node: usize,
        entity: &'static str,
        key: String,
        reason: String,
    },
}

impl DeleteRestriction {
    #[inline]
    pub fn node(&self) -> usize {
        match self {
            Self::ForeignKey { node, .. } | Self::Rule { node, .. } => *node,
        }
    }

    pub fn to_error(&self) -> Error {
        match self {
            Self::ForeignKey {
                entity,
                field,
                parent,
                key,
                count,
                ..
            } => Error::ForeignKeyRestricted {
                entity,
                field,
                parent,
                key: key.clone(),
                count: *count,
            },
            Self::Rule {
                entity,
                key,
                reason,
                ..
            } => Error::RemoveRestricted {
                entity,
                key: key.clone(),
                reason: reason.clone(),
            },
        }
    }
}

/// A node of the plan graph.
pub struct DeletePlanNode<'a> {
    pub index: usize,
    pub parent: Option<usize>,
    pub depth: usize,
    pub entity: &'static str,
    /// The foreign key field linking the node to its parent (`None` for the root).
    pub field: Option<&'static str>,
    pub action: DeleteAction,
    step: &'a dyn PlanStep,
}

impl DeletePlanNode<'_> {
    #[inline]
    pub fn key(&self) -> &dyn Debug {
        self.step.key_debug()
    }

    /// The typed key, i.e. `key_as::<<E as Entity>::Key>()`.
    #[inline]
    pub fn key_as<K: 'static>(&self) -> Option<&K> {
        self.step.key_any().downcast_ref()
    }
}

struct Node {
    parent: Option<usize>,
    depth: usize,
    step: Box<dyn PlanStep>,
}

pub struct DeletePlan {
    nodes: Vec<Node>,
    restrictions: Vec<DeleteRestriction>,
    done: usize,
}

impl DeletePlan {
    /// Builds the plan of the removal of `key` without modifying anything.
    pub async fn new<E>(trx: &mut CtxTransaction<'_>, key: E::Key) -> Result<Self>
    where
        E: EntityRemove + CtxTypeInfo,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'b> TransactionProvider<'b>: Delete<E>,
    {
        let mut plan = DeletePlan {
            nodes: Vec::new(),
            restrictions: Vec::new(),
            done: 0,
        };

        if entity_of::<E>(trx, &key).await?.is_none() {
            return Ok(plan);
        }

        let mut planner = DeletePlanner {
            node: 0,
            pending: Vec::new(),
            restrictions: Vec::new(),
            visited: FxHashMap::default(),
        };

        planner.visit::<E>(&key);
        plan.nodes.push(Node {
            parent: None,
            depth: 0,
            step: Box::new(RootStep::<E>(key)),
        });

        // parents are always before their children, so the plan executes in reverse order.
        let mut i = 0;

        while i < plan.nodes.len() {
            let node = &plan.nodes[i];

            if node.step.action() == DeleteAction::Remove {
                let ty = node.step.entity_type();
                planner.node = i;

                for rule in remove_rule::rules_of(ty) {
                    if let Some(reason) = rule.check(trx, node.step.key_any()).await? {
                        planner.restrictions.push(DeleteRestriction::Rule {
                            node: i,
                            entity: rule.entity,
                            key: format!("{:?}", node.step.key_debug()),
                            reason,
                        });
                    }
                }

                for f in planners_of(ty) {
                    f(trx, node.step.key_any(), &mut planner).await?;
                }

                let depth = node.depth + 1;

                plan.nodes
                    .extend(planner.pending.drain(..).map(|step| Node {
                        parent: Some(i),
                        depth,
                        step,
                    }));
            }

            i += 1;
        }

        plan.restrictions = planner.restrictions;
        Ok(plan)
    }

    #[inline]
    pub fn done(&self) -> usize {
        self.done
    }

    #[inline]
    pub fn is_done(&self) -> bool {
        self.done == self.nodes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    #[inline]
    pub fn is_restricted(&self) -> bool {
        !self.restrictions.is_empty()
    }

    /// Total number of steps.
    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// The graph of the plan. Parents are always listed before their children.
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = DeletePlanNode<'_>> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(index, n)| DeletePlanNode {
                index,
                parent: n.parent,
                depth: n.depth,
                entity: n.step.entity(),
                field: n.step.field(),
                action: n.step.action(),
                step: &*n.step,
            })
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.nodes.len() - self.done
    }

    #[inline]
    pub fn restrictions(&self) -> &[DeleteRestriction] {
        &self.restrictions
    }

    /// Number of steps by entity and action, in order of first appearance.
    pub fn summary(&self) -> Vec<(&'static str, DeleteAction, usize)> {
        let mut out: Vec<(&'static str, DeleteAction, usize)> = Vec::new();

        for n in &self.nodes {
            let (entity, action) = (n.step.entity(), n.step.action());

            match out
                .iter_mut()
                .find(|(e, a, _)| *e == entity && *a == action)
            {
                Some((_, _, count)) => *count += 1,
                None => out.push((entity, action, 1)),
            }
        }

        out
    }

    /// Executes up to `max_steps` steps (leaves first) in the transaction and returns the
    /// number of steps processed. Commit the transaction between calls to release the lock.
    ///
    /// Fails without changing anything when the plan is restricted. If the transaction
    /// fails to commit, build a new plan: the steps already done are no-ops.
    pub async fn execute(
        &mut self,
        trx: &mut CtxTransaction<'_>,
        max_steps: usize,
    ) -> Result<usize> {
        if let Some(r) = self.restrictions.first() {
            return Err(r.to_error());
        }

        let mut linked = FxHashMap::default();
        let mut done = self.done;
        let end = done.saturating_add(max_steps).min(self.nodes.len());

        while done < end {
            let i = self.nodes.len() - 1 - done;

            if self.is_linked(trx, i, &mut linked).await? {
                self.nodes[i].step.apply(trx).await?;
            }

            done += 1;
        }

        let count = done - self.done;
        self.done = done;
        Ok(count)
    }

    /// Checks the foreign key chain from the node up to the root.
    async fn is_linked(
        &self,
        trx: &CtxTransaction<'_>,
        mut i: usize,
        memo: &mut FxHashMap<usize, bool>,
    ) -> Result<bool> {
        let mut path = Vec::new();

        let mut linked = loop {
            if let Some(&v) = memo.get(&i) {
                break v;
            }

            path.push(i);

            match self.nodes[i].parent {
                Some(p) => i = p,
                None => break true,
            }
        };

        for &n in path.iter().rev() {
            linked = linked && self.nodes[n].step.is_linked(trx).await?;
            memo.insert(n, linked);
        }

        Ok(linked)
    }
}

/// Collects the children of the node being planned. Used by the planning handlers.
pub struct DeletePlanner {
    node: usize,
    pending: Vec<Box<dyn PlanStep>>,
    restrictions: Vec<DeleteRestriction>,
    visited: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl DeletePlanner {
    /// Returns `false` if the entity is already planned for removal.
    fn visit<E: Entity>(&mut self, key: &E::Key) -> bool {
        self.visited
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::new(FxHashSet::<E::Key>::default()))
            .downcast_mut::<FxHashSet<E::Key>>()
            .is_some_and(|set| set.insert(key.clone()))
    }
}

trait PlanStep: Send + Sync {
    fn action(&self) -> DeleteAction;
    fn entity(&self) -> &'static str;
    fn entity_type(&self) -> TypeId;
    fn field(&self) -> Option<&'static str>;
    fn key_any(&self) -> &(dyn Any + Send + Sync);
    fn key_debug(&self) -> &dyn Debug;

    /// Indicates if the entity still references its parent in the plan.
    fn is_linked<'a>(&'a self, trx: &'a CtxTransaction<'_>) -> BoxFuture<'a, Result<bool>>;

    fn apply<'a>(&'a self, trx: &'a mut CtxTransaction<'_>) -> BoxFuture<'a, Result<bool>>;
}

struct RootStep<E: Entity>(E::Key);

impl<E> PlanStep for RootStep<E>
where
    E: EntityRemove + CtxTypeInfo,
    ProviderContainer: LoadAll<E, (), E::Tbl>,
    for<'b> TransactionProvider<'b>: Delete<E>,
{
    fn action(&self) -> DeleteAction {
        DeleteAction::Remove
    }

    fn entity(&self) -> &'static str {
        E::NAME
    }

    fn entity_type(&self) -> TypeId {
        TypeId::of::<E>()
    }

    fn field(&self) -> Option<&'static str> {
        None
    }

    fn key_any(&self) -> &(dyn Any + Send + Sync) {
        &self.0
    }

    fn key_debug(&self) -> &dyn Debug {
        &self.0
    }

    fn is_linked<'a>(&'a self, _trx: &'a CtxTransaction<'_>) -> BoxFuture<'a, Result<bool>> {
        Box::pin(async { Ok(true) })
    }

    fn apply<'a>(&'a self, trx: &'a mut CtxTransaction<'_>) -> BoxFuture<'a, Result<bool>> {
        E::remove(trx, self.0.clone())
    }
}

struct ChildStep<F: ForeignKey> {
    parent: ParentKey<F>,
    key: ChildKey<F>,
    _f: PhantomData<F>,
}

impl<F: ForeignKey> ChildStep<F>
where
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    async fn linked_child<'a>(
        &'a self,
        trx: &'a CtxTransaction<'_>,
    ) -> Result<Option<&'a F::Child>> {
        Ok(entity_of::<F::Child>(trx, &self.key)
            .await?
            .filter(|c| F::foreign_key(&self.key, c) == Some(&self.parent)))
    }
}

struct Cascade<F: ForeignKey>(ChildStep<F>);

impl<F> PlanStep for Cascade<F>
where
    F: ForeignKey,
    F::Child: EntityRemove,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
    for<'b> TransactionProvider<'b>: Delete<F::Child>,
{
    fn action(&self) -> DeleteAction {
        DeleteAction::Remove
    }

    fn entity(&self) -> &'static str {
        F::Child::NAME
    }

    fn entity_type(&self) -> TypeId {
        TypeId::of::<F::Child>()
    }

    fn field(&self) -> Option<&'static str> {
        Some(F::FIELD)
    }

    fn key_any(&self) -> &(dyn Any + Send + Sync) {
        &self.0.key
    }

    fn key_debug(&self) -> &dyn Debug {
        &self.0.key
    }

    fn is_linked<'a>(&'a self, trx: &'a CtxTransaction<'_>) -> BoxFuture<'a, Result<bool>> {
        Box::pin(async move { Ok(self.0.linked_child(trx).await?.is_some()) })
    }

    fn apply<'a>(&'a self, trx: &'a mut CtxTransaction<'_>) -> BoxFuture<'a, Result<bool>> {
        F::Child::remove(trx, self.0.key.clone())
    }
}

struct SetNull<F: ForeignKey>(ChildStep<F>);

impl<F> PlanStep for SetNull<F>
where
    F: ForeignKeySetNull,
    F::Child: Clone,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    fn action(&self) -> DeleteAction {
        DeleteAction::SetNull
    }

    fn entity(&self) -> &'static str {
        F::Child::NAME
    }

    fn entity_type(&self) -> TypeId {
        TypeId::of::<F::Child>()
    }

    fn field(&self) -> Option<&'static str> {
        Some(F::FIELD)
    }

    fn key_any(&self) -> &(dyn Any + Send + Sync) {
        &self.0.key
    }

    fn key_debug(&self) -> &dyn Debug {
        &self.0.key
    }

    fn is_linked<'a>(&'a self, trx: &'a CtxTransaction<'_>) -> BoxFuture<'a, Result<bool>> {
        Box::pin(async move { Ok(self.0.linked_child(trx).await?.is_some()) })
    }

    fn apply<'a>(&'a self, trx: &'a mut CtxTransaction<'_>) -> BoxFuture<'a, Result<bool>> {
        Box::pin(async move {
            let Some(child) = self.0.linked_child(trx).await? else {
                return Ok(false);
            };

            let mut child = child.clone();
            F::set_null(&mut child);
            F::save(trx, self.0.key.clone(), child).await?;
            Ok(true)
        })
    }
}

/// Planning handler of the parent for [OnDelete::Cascade](crate::OnDelete::Cascade).
pub fn plan_cascade<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a (dyn Any + Send + Sync),
    planner: &'a mut DeletePlanner,
) -> BoxFuture<'a, Result<()>>
where
    F: ForeignKey,
    F::Child: EntityRemove,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
    for<'b> TransactionProvider<'b>: Delete<F::Child>,
{
    Box::pin(async move {
        let Some(parent) = key.downcast_ref::<ParentKey<F>>() else {
            return Ok(());
        };

        for key in children_of::<F>(trx, parent).await? {
            if planner.visit::<F::Child>(&key) {
                planner.pending.push(Box::new(Cascade::<F>(ChildStep {
                    parent: parent.clone(),
                    key,
                    _f: PhantomData,
                })));
            }
        }

        Ok(())
    })
}

/// Planning handler of the parent for [OnDelete::Restrict](crate::OnDelete::Restrict).
pub fn plan_restrict<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a (dyn Any + Send + Sync),
    planner: &'a mut DeletePlanner,
) -> BoxFuture<'a, Result<()>>
where
    F: ForeignKey,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let Some(parent) = key.downcast_ref::<ParentKey<F>>() else {
            return Ok(());
        };

        let count = count_children::<F>(trx, parent).await?;

        if count > 0 {
            planner.restrictions.push(DeleteRestriction::ForeignKey {
                node: planner.node,
                entity: F::Child::NAME,
                field: F::FIELD,
                parent: F::Parent::NAME,
                key: debug_string(parent),
                count,
            });
        }

        Ok(())
    })
}

/// Planning handler of the parent for [OnDelete::SetNull](crate::OnDelete::SetNull).
pub fn plan_set_null<'a, F>(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a (dyn Any + Send + Sync),
    planner: &'a mut DeletePlanner,
) -> BoxFuture<'a, Result<()>>
where
    F: ForeignKeySetNull,
    F::Child: Clone,
    Ctx: AsRefAsync<<F::Child as EntityAccessor>::Tbl>,
    ProviderContainer: LoadAll<F::Child, (), <F::Child as EntityAccessor>::Tbl>,
{
    Box::pin(async move {
        let Some(parent) = key.downcast_ref::<ParentKey<F>>() else {
            return Ok(());
        };

        for key in children_of::<F>(trx, parent).await? {
            planner.pending.push(Box::new(SetNull::<F>(ChildStep {
                parent: parent.clone(),
                key,
                _f: PhantomData,
            })));
        }

        Ok(())
    })
}
