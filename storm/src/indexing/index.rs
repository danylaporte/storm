use fast_set::AdaptiveBitmapLog;

use crate::Entity;
use std::any::{type_name, Any, TypeId};

pub trait Index<E>: Any + Send + Sync {
    fn apply_log(&mut self, log: Box<dyn IndexLog>);

    fn create_log(&self) -> Box<dyn IndexLog>;

    fn remove(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E)
    where
        E: Entity;

    fn upsert(&self, log: &mut dyn IndexLog, k: &E::Key, entity: &E, old: Option<&E>)
    where
        E: Entity;
}

pub trait IndexTrx {
    type Trx<'a>
    where
        Self: 'a;

    fn trx<'a>(&'a self, log: &'a dyn IndexLog) -> Self::Trx<'a>;
}

pub trait IndexLog: Any + Send + Sync {}

impl IndexLog for AdaptiveBitmapLog {}

pub struct IndexList<E>(Vec<Box<dyn Index<E>>>);

impl<E> Default for IndexList<E>
where
    E: 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<E> IndexList<E>
where
    E: 'static,
{
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn apply_changes(&mut self, logs: IndexLogs) {
        for (index, log) in self.0.iter_mut().zip(logs) {
            index.apply_log(log);
        }
    }

    #[track_caller]
    pub fn get<I>(&self) -> (&I, usize)
    where
        E: 'static,
        I: Index<E> + 'static,
    {
        for (index, i) in self.0.iter().enumerate() {
            if let Some(v) = <dyn Any>::downcast_ref(&**i) {
                return (v, index);
            }
        }

        panic!("Index {} not registered", type_name::<I>());
    }

    fn get_or_init_log<'a>(&self, logs: &'a mut IndexLogs, i: usize) -> &'a mut dyn IndexLog {
        if logs.len() <= i {
            logs.reserve_exact(self.0.len() - logs.len());

            for i in logs.len()..self.0.len() {
                logs.push((*self.0[i]).create_log());
            }

            assert!(i < self.0.len());
        }

        &mut **unsafe { logs.get_unchecked_mut(i) }
    }

    #[track_caller]
    pub fn register<I>(&mut self, index: I)
    where
        E: 'static,
        I: Index<E> + 'static,
    {
        let tid = TypeId::of::<I>();

        if self.0.iter().any(|i| (**i).type_id() == tid) {
            panic!("Index {} already registered", type_name::<I>());
        }

        self.0.push(Box::new(index));
    }

    pub fn remove(&self, logs: &mut IndexLogs, key: &E::Key, entity: &E)
    where
        E: Entity,
    {
        for (i, index) in self.0.iter().enumerate() {
            let log = self.get_or_init_log(logs, i);
            index.remove(log, key, entity);
        }
    }

    #[track_caller]
    pub fn trx<'a, I>(&'a self, log: &'a mut IndexLogs) -> I::Trx<'a>
    where
        E: 'static,
        I: Index<E> + IndexTrx,
    {
        let (index, log_index) = self.get::<I>();
        let log = &mut *log[log_index];

        index.trx(log)
    }

    pub fn upsert(&self, logs: &mut IndexLogs, key: &E::Key, entity: &E, old: Option<&E>)
    where
        E: Entity,
    {
        for (i, index) in self.0.iter().enumerate() {
            let log = self.get_or_init_log(logs, i);
            index.upsert(log, key, entity, old);
        }
    }
}

pub(crate) type IndexLogs = Vec<Box<dyn IndexLog>>;

#[test]
fn check_index_list_get() {
    struct Idx;

    impl<E> Index<E> for Idx {
        fn apply_log(&mut self, _log: Box<dyn IndexLog>) {
            todo!();
        }

        fn create_log(&self) -> Box<dyn IndexLog> {
            todo!();
        }

        fn remove(&self, _log: &mut dyn IndexLog, _k: &<E>::Key, _entity: &E)
        where
            E: Entity,
        {
            todo!()
        }

        fn upsert(&self, _log: &mut dyn IndexLog, _k: &<E>::Key, _entity: &E, _old: Option<&E>)
        where
            E: Entity,
        {
            todo!()
        }
    }

    let mut list = IndexList::<()>::new();

    list.register(Idx);

    let _ = list.get::<Idx>();
}
