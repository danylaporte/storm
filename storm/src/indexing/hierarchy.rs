use super::IndexLog;
use crate::{
    indexing::{set::empty, Index, IndexTrx, Set},
    Entity,
};
use nohash::IntMap;
use roaring::RoaringBitmap;
use std::{any::Any, collections::hash_map::Entry, marker::PhantomData};

type IntSet = nohash::IntSet<u32>;

pub struct HierarchyIndex<K, A> {
    children: IntMap<u32, RoaringBitmap>,
    cycles: IntSet,
    descendants: IntMap<u32, RoaringBitmap>,
    parents: IntMap<u32, u32>,
    _ka: PhantomData<(K, A)>,
}

impl<K, A> HierarchyIndex<K, A>
where
    K: 'static,
    A: 'static,
{
    pub fn new() -> Self {
        Self {
            children: Default::default(),
            cycles: Default::default(),
            descendants: Default::default(),
            parents: Default::default(),
            _ka: PhantomData,
        }
    }

    pub fn ancestors(&self, child: K) -> AncestorIndexIter<'_, K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        let mut iter = self.ancestors_with_self(child);
        iter.next();
        iter
    }

    pub fn ancestors_with_self(&self, child: K) -> AncestorIndexIter<'_, K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        AncestorIndexIter {
            child: Some(usize::from(child) as u32),
            cycles: &self.cycles,
            parents: &self.parents,
            _k: PhantomData,
        }
    }

    #[inline]
    pub fn children(&self, parent: K) -> &Set<K>
    where
        usize: From<K>,
    {
        Set::from_roaring_bitmap_ref(self.children_impl(usize::from(parent)))
    }

    fn children_impl(&self, parent: usize) -> &RoaringBitmap {
        self.children.get(&(parent as u32)).unwrap_or_else(empty)
    }

    #[inline]
    pub fn cycles(&self) -> impl Iterator<Item = K> + '_
    where
        K: TryFrom<usize>,
    {
        self.cycles
            .iter()
            .filter_map(|v| K::try_from(*v as usize).ok())
    }

    pub fn depth(&self, node: K) -> Result<usize, CycleError>
    where
        usize: From<K>,
    {
        self.depth_impl(usize::from(node) as u32)
    }

    fn depth_impl(&self, node: u32) -> Result<usize, CycleError> {
        let mut cursor = Some(node);
        let mut depth = 0;

        while let Some(current) = cursor {
            if self.cycles.contains(&current) {
                return Err(CycleError(current));
            }

            depth += 1;
            cursor = self.parents.get(&current).copied();
        }

        Ok(depth)
    }

    #[inline]
    pub fn descendants(&self, parent: K) -> &Set<K>
    where
        usize: From<K>,
    {
        Set::from_roaring_bitmap_ref(self.descendants_impl(usize::from(parent)))
    }

    fn descendants_impl(&self, parent: usize) -> &RoaringBitmap {
        self.descendants.get(&(parent as u32)).unwrap_or_else(empty)
    }

    #[inline]
    pub fn has_cycle(&self, node: K) -> bool
    where
        usize: From<K>,
    {
        self.has_cycle_impl(usize::from(node) as u32)
    }

    #[inline]
    fn has_cycle_impl(&self, node: u32) -> bool {
        self.cycles.contains(&node)
    }

    #[inline]
    pub fn is_descendant_of(&self, child: K, parent: K) -> bool
    where
        usize: From<K>,
    {
        self.is_descendant_of_impl(usize::from(child), usize::from(parent))
    }

    fn is_descendant_of_impl(&self, child: usize, parent: usize) -> bool {
        self.descendants_impl(parent).contains(child as u32)
    }

    #[inline]
    pub fn parent(&self, child: K) -> Option<K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        self.parent_impl(usize::from(child) as u32)
            .and_then(|p| K::try_from(p as usize).ok())
    }

    fn parent_impl(&self, child: u32) -> Option<u32> {
        self.parents.get(&child).copied()
    }

    fn apply(&mut self, log: HierarchyLog) {
        if let Some(cycles) = log.cycles {
            self.cycles = cycles;
        }

        // Apply changes
        for (parent, set) in log.descendants {
            if set.is_empty() {
                self.descendants.remove(&parent);
            } else {
                self.descendants.insert(parent, set);
            }
        }

        for (parent, set) in log.children {
            if set.is_empty() {
                self.children.remove(&parent);
            } else {
                self.children.insert(parent, set);
            }
        }

        for (child, parent_opt) in log.parents {
            match parent_opt {
                Some(p) => {
                    self.parents.insert(child, p);
                }
                None => {
                    self.parents.remove(&child);
                }
            }
        }
    }

    fn upsert_impl(
        &self,
        log: &mut dyn IndexLog,
        old: Option<(u32, Option<u32>)>,
        new: Option<(u32, Option<u32>)>,
    ) {
        if old != new {
            let mut trx = HierarchyTrx {
                index: self,
                log: hierarchy_log_mut(log),
            };

            if let Some(old) = old {
                trx.remove(old.0);
            }

            if let Some((child, parent)) = new {
                trx.insert(parent, child);
            }
        }
    }

    fn remove_impl(&self, log: &mut dyn IndexLog, old: Option<(u32, Option<u32>)>) {
        if let Some(old) = old {
            HierarchyTrx {
                index: self,
                log: hierarchy_log_mut(log),
            }
            .remove(old.0);
        }
    }
}

impl<K, A> Default for HierarchyIndex<K, A>
where
    K: 'static,
    A: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, A> FromIterator<(Option<K>, K)> for HierarchyIndex<K, A>
where
    K: 'static,
    A: 'static,
    usize: From<K>,
{
    fn from_iter<T: IntoIterator<Item = (Option<K>, K)>>(iter: T) -> Self {
        let mut index = HierarchyIndex::new();
        let mut log = HierarchyLog::default();

        let mut trx = HierarchyTrx {
            index: &index,
            log: &mut log,
        };

        for (parent, child) in iter {
            trx.insert(
                parent.map(|p| usize::from(p) as u32),
                usize::from(child) as u32,
            );
        }

        index.apply(log);
        index
    }
}

impl<E, K, A> Index<E> for HierarchyIndex<K, A>
where
    E: Entity,
    K: Send + Sync + 'static,
    A: HierarchyAdapt<E, K> + Send + Sync + 'static,
    usize: From<K>,
{
    fn apply_log(&mut self, log: Box<dyn IndexLog>) {
        let log = *Box::<dyn Any>::downcast(log).expect("HierarchyLog");

        self.apply(log);
    }

    fn create_log(&self) -> Box<dyn IndexLog> {
        Box::new(HierarchyLog::default())
    }

    fn upsert(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E, old: Option<&E>)
    where
        E: Entity,
    {
        let old = old.and_then(|old| adapt_u32::<A, E, K>(k, old));
        let new = adapt_u32::<A, E, K>(k, entity);

        self.upsert_impl(log, old, new);
    }

    fn remove(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E)
    where
        E: Entity,
    {
        let old = adapt_u32::<A, E, K>(k, entity);

        self.remove_impl(log, old);
    }
}

fn hierarchy_log_mut(log: &mut dyn IndexLog) -> &mut HierarchyLog {
    <dyn Any>::downcast_mut(&mut *log).expect("IndexMap")
}

#[derive(Debug, Default)]
pub struct HierarchyLog {
    children: IntMap<u32, RoaringBitmap>,
    cycles: Option<IntSet>,
    descendants: IntMap<u32, RoaringBitmap>,
    parents: IntMap<u32, Option<u32>>,
}

impl HierarchyLog {
    #[inline]
    fn cycles<'a, K, A>(&'a self, index: &'a HierarchyIndex<K, A>) -> &'a IntSet {
        self.cycles.as_ref().unwrap_or(&index.cycles)
    }

    fn cycles_mut<K, A>(&mut self, index: &HierarchyIndex<K, A>) -> &mut IntSet {
        if self.cycles.is_none() {
            self.cycles = Some(index.cycles.clone());
        }

        unsafe { self.cycles.as_mut().unwrap_unchecked() }
    }

    fn parent<K, A>(&self, index: &HierarchyIndex<K, A>, child: u32) -> Option<u32> {
        match self.parents.get(&child) {
            Some(Some(parent)) => Some(*parent), // Explicit parent set in log
            Some(None) => None,                  // Explicitly removed in log
            None => index.parents.get(&child).copied(), // Not in log, check index
        }
    }
}

impl IndexLog for HierarchyLog {}

pub struct HierarchyTrx<'a, K, A> {
    index: &'a HierarchyIndex<K, A>,
    log: &'a mut HierarchyLog,
}

impl<K, A> HierarchyTrx<'_, K, A>
where
    K: 'static,
    A: 'static,
{
    /// Returns an iterator over ancestors, stops at cycle nodes
    pub fn ancestors(&self, child: K) -> AncestorTrxIter<'_, K, A>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        let mut iter = self.ancestors_with_self(child);
        iter.next();
        iter
    }

    pub fn ancestors_with_self(&self, child: K) -> AncestorTrxIter<'_, K, A>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        AncestorTrxIter {
            child: Some(usize::from(child) as u32),
            cycles: self.log.cycles(self.index),
            index: self.index,
            log: self.log,
        }
    }

    pub fn children(&self, node: K) -> &Set<K>
    where
        usize: From<K>,
    {
        Set::from_roaring_bitmap_ref(
            self.children_impl(usize::from(node) as u32)
                .unwrap_or_else(empty),
        )
    }

    fn children_impl(&self, node: u32) -> Option<&RoaringBitmap> {
        self.log
            .children
            .get(&node)
            .or_else(|| self.index.children.get(&node))
    }

    fn children_mut(&mut self, parent: u32) -> &mut RoaringBitmap {
        self.log.children.entry(parent).or_insert_with(|| {
            self.index
                .children
                .get(&parent)
                .cloned()
                .unwrap_or_default()
        })
    }

    /// Get a list of node having a cycle.
    #[inline]
    pub fn cycles(&self) -> impl Iterator<Item = K> + '_
    where
        K: TryFrom<usize>,
    {
        self.log
            .cycles(self.index)
            .iter()
            .filter_map(|v| K::try_from(*v as usize).ok())
    }

    #[inline]
    pub fn depth(&self, node: K) -> Result<usize, CycleError>
    where
        usize: From<K>,
    {
        self.depth_impl(usize::from(node) as u32)
    }

    fn depth_impl(&self, node: u32) -> Result<usize, CycleError> {
        let mut cursor = Some(node);
        let mut depth = 0;
        let cycles = self.log.cycles(self.index);

        while let Some(current) = cursor {
            if cycles.contains(&current) {
                return Err(CycleError(current));
            }

            depth += 1;
            cursor = self.parent_impl(current);
        }

        Ok(depth)
    }

    pub fn descendants(&self, parent: K) -> &Set<K>
    where
        usize: From<K>,
    {
        Set::from_roaring_bitmap_ref(self.descendants_impl(usize::from(parent)))
    }

    fn descendants_impl(&self, parent: usize) -> &RoaringBitmap {
        self.log
            .descendants
            .get(&(parent as u32))
            .unwrap_or_else(|| self.index.descendants_impl(parent))
    }

    fn descendants_mut(&mut self, parent: u32) -> &mut RoaringBitmap {
        self.log.descendants.entry(parent).or_insert_with(|| {
            self.index
                .descendants
                .get(&parent)
                .cloned()
                .unwrap_or_default()
        })
    }

    fn detect_cycle_from_node(&mut self, start_node: u32) -> Option<u32> {
        let mut visited = IntSet::default();
        let mut current = Some(start_node);

        while let Some(node) = current {
            if !visited.insert(node) {
                return Some(node);
            }

            current = self.parent_impl(node);
        }

        None
    }

    fn get_descendants_including_self(&self, node: u32) -> IntSet {
        let mut set = self.get_strict_descendants(node);
        set.insert(node);
        set
    }

    fn get_strict_descendants(&self, node: u32) -> IntSet {
        let mut all_descendants = IntSet::default();
        let mut queue = Vec::new();

        if let Some(children) = self.children_impl(node) {
            queue.extend(children);
        }

        while let Some(current) = queue.pop() {
            if all_descendants.insert(current) {
                if let Some(children) = self.children_impl(current) {
                    queue.extend(children);
                }
            }
        }

        all_descendants
    }

    #[inline]
    pub fn has_cycle(&self, id: K) -> bool
    where
        usize: From<K>,
    {
        self.has_cycle_impl(usize::from(id) as u32)
    }

    #[inline]
    fn has_cycle_impl(&self, node: u32) -> bool {
        self.log.cycles(self.index).contains(&node)
    }

    fn insert(&mut self, parent: Option<u32>, child: u32) {
        let old_parent = self.parent_impl(child);

        // Only do reparenting work if the parent is actually changing
        if old_parent != parent {
            // If there was an old parent, clean up the old relationship
            if let Some(old_p) = old_parent {
                // Remove child from old parent's children
                self.children_mut(old_p).remove(child);

                // Get all nodes that are moving (child + its descendants)
                let moving_nodes = self.get_descendants_including_self(child);

                // Remove these nodes from all ancestors of the old parent
                self.update_ancestor_descendants_remove(old_p, &moving_nodes);
            }

            // Set the new parent
            self.log.parents.insert(child, parent);

            // If there's a new parent, set up the new relationship
            if let Some(new_p) = parent {
                // Add child to new parent's children
                self.children_mut(new_p).insert(child);

                // Get all nodes that are moving (child + its descendants)
                let child_descendants = self.get_strict_descendants(child);

                // Add these nodes to all ancestors of the new parent
                self.update_ancestor_descendants_add(new_p, child, &child_descendants);

                // Check for cycles after the insertion
                if let Some(cycle_node) = self.detect_cycle_from_node(child) {
                    self.log.cycles_mut(self.index).insert(cycle_node);
                }
            }
        }
    }

    #[inline]
    pub fn is_descendant_of(&self, child: K, parent: K) -> bool
    where
        usize: From<K>,
    {
        self.is_descendant_of_impl(usize::from(child), usize::from(parent))
    }

    fn is_descendant_of_impl(&self, child: usize, parent: usize) -> bool {
        self.descendants_impl(parent).contains(child as u32)
    }

    pub fn parent(&self, child: K) -> Option<K>
    where
        K: TryFrom<usize>,
        usize: From<K>,
    {
        self.parent_impl(usize::from(child) as u32)
            .and_then(|p| K::try_from(p as usize).ok())
    }

    fn parent_impl(&self, child: u32) -> Option<u32> {
        self.log.parent(self.index, child)
    }

    fn remove(&mut self, node: u32) {
        // Get all the information we need BEFORE making any changes
        let nodes_to_remove = self.get_descendants_including_self(node);

        // Collect parent-child relationships that need to be broken
        let mut parent_child_pairs = Vec::new();

        for &node_to_remove in &nodes_to_remove {
            if let Some(parent) = self.parent_impl(node_to_remove) {
                parent_child_pairs.push((parent, node_to_remove));
            }
        }

        // Collect ancestors that need their descendants updated
        let mut ancestors = Vec::new();
        let mut cursor = self.parent_impl(node);

        while let Some(ancestor) = cursor {
            ancestors.push(ancestor);
            cursor = self.parent_impl(ancestor);
        }

        // Now make all the changes:

        // 1. Break parent-child relationships
        for (parent, child) in parent_child_pairs {
            self.children_mut(parent).remove(child);
        }

        // 2. Set all removed nodes' parents to None and clear their children/descendants
        for &node_to_remove in &nodes_to_remove {
            self.log.parents.insert(node_to_remove, None);
            set_map_to_empty(&mut self.log.children, &self.index.children, node_to_remove);
            set_map_to_empty(
                &mut self.log.descendants,
                &self.index.descendants,
                node_to_remove,
            );
            // Remove from detected cycles since the node is being removed
            self.log.cycles_mut(self.index).remove(&node_to_remove);
        }

        // 3. Update ancestor descendants
        for ancestor in ancestors {
            let descendants_set = self.descendants_mut(ancestor);

            for &removed_node in &nodes_to_remove {
                descendants_set.remove(removed_node);
            }
        }
    }

    fn update_ancestor_descendants_remove(
        &mut self,
        start_ancestor: u32,
        nodes_to_remove: &IntSet,
    ) {
        let mut visited = IntSet::default();
        let mut cursor = Some(start_ancestor);

        while let Some(ancestor) = cursor {
            if !visited.insert(ancestor) {
                break; // Cycle detection
            }

            let descendants_set = self.descendants_mut(ancestor);

            for &node in nodes_to_remove {
                descendants_set.remove(node);
            }

            cursor = self.parent_impl(ancestor);
        }
    }

    fn update_ancestor_descendants_add(
        &mut self,
        start_ancestor: u32,
        child: u32,
        descendants: &IntSet,
    ) {
        let mut visited = IntSet::default();
        let mut cursor = Some(start_ancestor);

        while let Some(ancestor) = cursor {
            if !visited.insert(ancestor) {
                break; // Cycle detection
            }

            let set = self.descendants_mut(ancestor);
            set.extend(descendants.iter().copied());
            set.insert(child);
            cursor = self.parent_impl(ancestor);
        }
    }
}

impl<K, A> IndexTrx for HierarchyIndex<K, A> {
    type Trx<'a>
        = HierarchyTrx<'a, K, A>
    where
        Self: 'a;

    fn trx<'a>(&'a self, log: &'a mut dyn IndexLog) -> Self::Trx<'a> {
        HierarchyTrx {
            index: self,
            log: hierarchy_log_mut(log),
        }
    }
}

#[derive(Debug)]
pub enum HierarchyError {
    NodeCycle(u32),
}

#[derive(Debug, PartialEq)]
pub struct CycleError(u32);

pub struct AncestorIndexIter<'a, K> {
    child: Option<u32>,
    cycles: &'a IntSet,
    parents: &'a IntMap<u32, u32>,
    _k: PhantomData<K>,
}

impl<K> AncestorIndexIter<'_, K> {
    fn next_impl(&mut self) -> Option<u32> {
        let item = self.child.take();

        self.child = item.as_ref().and_then(|c| {
            if self.cycles.contains(c) {
                None
            } else {
                self.parents.get(c).copied()
            }
        });

        item
    }
}

impl<K> Iterator for AncestorIndexIter<'_, K>
where
    K: TryFrom<usize>,
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_impl().map(|v| match K::try_from(v as usize) {
            Ok(v) => v,
            Err(_) => unreachable!("Cannot convert to K"),
        })
    }
}

pub struct AncestorTrxIter<'a, K, A> {
    child: Option<u32>,
    cycles: &'a IntSet,
    index: &'a HierarchyIndex<K, A>,
    log: &'a HierarchyLog,
}

impl<K, A> AncestorTrxIter<'_, K, A> {
    fn next_impl(&mut self) -> Option<u32> {
        let item = self.child.take();

        self.child = item.and_then(|child| {
            if self.cycles.contains(&child) {
                None
            } else {
                self.log.parent(self.index, child)
            }
        });

        item
    }
}

impl<K, A> Iterator for AncestorTrxIter<'_, K, A>
where
    K: TryFrom<usize>,
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_impl().map(|v| match K::try_from(v as usize) {
            Ok(v) => v,
            Err(_) => unreachable!("Cannot convert to K"),
        })
    }
}

fn set_map_to_empty(
    map: &mut IntMap<u32, RoaringBitmap>,
    index_map: &IntMap<u32, RoaringBitmap>,
    key: u32,
) {
    let exists_in_index = index_map.contains_key(&key);

    match map.entry(key) {
        Entry::Occupied(mut o) => {
            if exists_in_index {
                o.get_mut().clear();
            } else {
                o.remove();
            }
        }
        Entry::Vacant(v) => {
            if exists_in_index {
                v.insert(Default::default());
            }
        }
    }
}

fn adapt_u32<A, E, K>(k: &E::Key, e: &E) -> Option<(u32, Option<u32>)>
where
    A: HierarchyAdapt<E, K>,
    E: Entity,
    usize: From<K>,
{
    A::adapt(k, e).map(|(child, parent)| {
        (
            usize::from(child) as u32,
            parent.map(|p| usize::from(p) as u32),
        )
    })
}

pub trait HierarchyAdapt<E: Entity, K> {
    fn adapt(k: &E::Key, v: &E) -> Option<(K, Option<K>)>;
}

#[macro_export]
macro_rules! hierarchy_adapt {
    ($adapt:ident, $alias:ident, $init:ident, $vis:vis fn $n:ident($id:ident: &$entity_key:ty, $entity:ident: &$entity_ty:ty $(,)?) -> Option<($k:ty, Option<$k1:ty>)> {
        $($t:tt)*
    }) => {
        $vis struct $adapt;

        impl storm::indexing::HierarchyAdapt<$entity_ty, $k> for $adapt {
            #[allow(unused_variables)]
            fn adapt($id: &$entity_key, $entity: &$entity_ty) -> Option<($k, Option<$k>)> {
                $($t)*
            }
        }

        $vis type $alias = storm::indexing::HierarchyIndex<$k, $adapt>;

        #[$crate::linkme::distributed_slice($crate::STORM_INITS)]
        #[linkme(crate = $crate::linkme)]
        fn $init() {
            <$entity_ty as $crate::EntityAccessor>::entity_inits().register(|tbl| {
                tbl.register_index($alias::new());
            });
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_index() {
        let mut idx = HierarchyIndex::<usize, ()>::new();
        let mut log = HierarchyLog::default();

        {
            let trx = idx.trx(&mut log);

            assert!(trx.children(1).iter().next().is_none());
            assert!(trx.descendants(1).iter().next().is_none());
            assert!(trx.ancestors(1).next().is_none());
            assert_eq!(trx.depth(1).unwrap(), 1);
        }

        idx.apply(log);

        assert!(idx.children(1).iter().next().is_none());
        assert!(idx.descendants(1).iter().next().is_none());
        assert!(idx.ancestors(1).next().is_none());
        assert_eq!(idx.depth(1).unwrap(), 1);
    }

    #[test]
    fn test_cycle_detection() {
        let mut idx = HierarchyIndex::<usize, ()>::new();
        let mut log = HierarchyLog::default();

        {
            let mut trx = idx.trx(&mut log);
            trx.insert(None, 1);
            trx.insert(Some(1), 2);
            trx.insert(Some(2), 3);

            // cycle: 3 → 1
            trx.insert(Some(3), 1);

            assert!(trx.depth(1).is_err());
            assert!(trx.depth(2).is_err());
            assert!(trx.depth(3).is_err());
        }

        idx.apply(log);

        assert!(idx.depth(1).is_err());
        assert!(idx.depth(2).is_err());
        assert!(idx.depth(3).is_err());
    }
}
