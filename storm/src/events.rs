use crate::{registry::InitCell, BoxFuture, Ctx, CtxTransaction, Entity, Result};

type EventInner<T> = InitCell<Vec<T>>;

pub struct AppliedEvent<E: Entity>(EventInner<AppliedEventFn<E>>);

impl<E: Entity> AppliedEvent<E> {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub fn call(&'static self, key: &E::Key, old: Option<&E>, new: Option<&E>) {
        for f in self.0.get() {
            f(key, old, new);
        }
    }

    pub fn on(&'static self, f: AppliedEventFn<E>) {
        self.0.get_mut().push(f);
    }
}

impl<E: Entity> Default for AppliedEvent<E> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type AppliedEventFn<E> = fn(key: &<E as Entity>::Key, old: Option<&E>, new: Option<&E>);

pub struct ChangedEvent<T>(EventInner<ChangedEventFn<T>>);

impl<T> ChangedEvent<T> {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub async fn call<'a>(
        &'static self,
        trx: &'a mut CtxTransaction<'_>,
        old: &'a Option<T>,
        new: &'a Option<T>,
    ) -> Result<()> {
        for f in self.0.get() {
            f(trx, old, new).await?;
        }

        Ok(())
    }

    pub fn on(&'static self, f: ChangedEventFn<T>) {
        self.0.get_mut().push(f);
    }
}

impl<T> Default for ChangedEvent<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type ChangedEventFn<T> = for<'a> fn(
    trx: &'a mut CtxTransaction<'_>,
    old: &'a Option<T>,
    new: &'a Option<T>,
) -> BoxFuture<'a, Result<()>>;

pub struct ClearEvent(EventInner<ClearEventFn>);

impl ClearEvent {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub fn call(&'static self, ctx: &mut Ctx) {
        for f in self.0.get() {
            f(ctx);
        }
    }

    pub fn on(&'static self, f: ClearEventFn) {
        self.0.get_mut().push(f);
    }
}

impl Default for ClearEvent {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type ClearEventFn = fn(ctx: &mut Ctx);

pub struct GcEvent(EventInner<GcEventFn>);

impl GcEvent {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub fn call(&'static self, ctx: &mut Ctx) {
        for f in self.0.get() {
            f(ctx);
        }
    }

    pub fn on(&'static self, f: GcEventFn) {
        self.0.get_mut().push(f);
    }
}

impl Default for GcEvent {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type GcEventFn = fn(ctx: &mut Ctx);

pub struct RemovedEvent<E: Entity>(EventInner<RemovedEventFn<E>>);

impl<E: Entity> RemovedEvent<E> {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub async fn call<'a>(
        &'static self,
        trx: &'a mut CtxTransaction<'_>,
        id: &'a <E as Entity>::Key,
        entity: &'a E,
    ) -> Result<()> {
        for f in self.0.get() {
            f(trx, id, entity).await?;
        }

        Ok(())
    }

    pub fn on(&'static self, f: RemovedEventFn<E>) {
        self.0.get_mut().push(f);
    }
}

impl<E: Entity> Default for RemovedEvent<E> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type RemovedEventFn<E> = for<'a> fn(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a <E as Entity>::Key,
    old: &'a E,
) -> BoxFuture<'a, Result<()>>;

pub struct RemovingEvent<E: Entity>(EventInner<RemovingEventFn<E>>);

impl<E: Entity> RemovingEvent<E> {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub async fn call<'a>(
        &'static self,
        trx: &'a mut CtxTransaction<'_>,
        id: &'a <E as Entity>::Key,
    ) -> Result<()> {
        for f in self.0.get() {
            f(trx, id).await?;
        }

        Ok(())
    }

    pub fn on(&'static self, f: RemovingEventFn<E>) {
        self.0.get_mut().push(f);
    }
}

impl<E: Entity> Default for RemovingEvent<E> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type RemovingEventFn<E> = for<'a> fn(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a <E as Entity>::Key,
) -> BoxFuture<'a, Result<()>>;

pub struct TouchedEvent(EventInner<TouchedEventFn>);

impl TouchedEvent {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub fn call(&'static self, ctx: &mut Ctx) {
        for f in self.0.get() {
            f(ctx);
        }
    }

    pub fn on(&'static self, f: TouchedEventFn) {
        self.0.get_mut().push(f);
    }
}

impl Default for TouchedEvent {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type TouchedEventFn = fn(ctx: &mut Ctx);

pub struct UpsertedEvent<E: Entity>(EventInner<UpsertedEventFn<E>>);

impl<E: Entity> UpsertedEvent<E> {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub async fn call<'a>(
        &'static self,
        trx: &'a mut CtxTransaction<'_>,
        id: &'a <E as Entity>::Key,
        old: Option<&'a E>,
    ) -> Result<()> {
        for f in self.0.get() {
            f(trx, id, old).await?;
        }

        Ok(())
    }

    pub fn on(&'static self, f: UpsertedEventFn<E>) {
        self.0.get_mut().push(f);
    }
}

impl<E: Entity> Default for UpsertedEvent<E> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type UpsertedEventFn<E> = for<'a> fn(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a <E as Entity>::Key,
    old: Option<&'a E>,
) -> BoxFuture<'a, Result<()>>;

pub struct UpsertingEvent<E: Entity>(EventInner<UpsertingEventFn<E>>);

impl<E: Entity> UpsertingEvent<E> {
    pub const fn new() -> Self {
        Self(EventInner::new(Vec::new()))
    }

    pub async fn call<'a>(
        &'static self,
        trx: &'a mut CtxTransaction<'_>,
        id: &'a <E as Entity>::Key,
        entity: &'a E,
    ) -> Result<()> {
        for f in self.0.get() {
            f(trx, id, entity).await?;
        }

        Ok(())
    }

    pub fn on(&'static self, f: UpsertingEventFn<E>) {
        self.0.get_mut().push(f);
    }
}

impl<E: Entity> Default for UpsertingEvent<E> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

type UpsertingEventFn<E> = for<'a> fn(
    trx: &'a mut CtxTransaction<'_>,
    key: &'a <E as Entity>::Key,
    new: &'a E,
) -> BoxFuture<'a, Result<()>>;
