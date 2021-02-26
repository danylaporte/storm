use async_cell_lock::QueueRwLock;
use cache::Cache;
use storm::{prelude::*, Connected, Ctx, Entity, GetVersion, OnceCell, Result, Version};
use vec_map::VecMap;

fn create_ctx() -> QueueRwLock<Connected<Ctx, ()>> {
    QueueRwLock::new(Connected {
        ctx: Ctx::default(),
        provider: (),
    })
}

#[tokio::test]
async fn flow() -> Result<()> {
    let ctx = create_ctx();

    let ctx = ctx.read().await;
    let ctx = ctx.queue().await;

    let trx = ctx.transaction().await?;
    let log = trx.commit().await?;

    let mut ctx = ctx.write().await;

    ctx.apply_log(log);

    let _ = ctx.read().await;

    Ok(())
}

#[tokio::test]
async fn read() -> Result<()> {
    let ctx = create_ctx();

    let ctx = ctx.read().await;

    // once_cell<VecMap<_>>
    let oc_vm = ctx.oc_vm().await?;
    let _ = oc_vm.get(&0).is_none();
    let _ = oc_vm.into_iter();

    // once_cell<Version<VecMap<_>>>
    let oc_v_vm = ctx.oc_v_vm().await?;
    let _ = oc_v_vm.get(&0).is_none();
    let _ = oc_v_vm.get_version();
    let _ = oc_v_vm.into_iter();

    // Cache<_>
    let c = ctx.c();
    let _ = c.get(&0).is_none();
    let _ = c.into_iter();

    // Version<Cache<_>>
    let v_c = ctx.v_c();
    let _ = v_c.get(&0).is_none();
    let _ = v_c.get_version();
    let _ = v_c.into_iter();

    Ok(())
}

#[tokio::test]
async fn transaction() -> Result<()> {
    let ctx = create_ctx();

    let ctx = ctx.read().await.queue().await;

    let mut trx = ctx.transaction().await?;

    // once_cell<VecMap<_>>
    let oc_vm = trx.oc_vm().await?;
    let _ = oc_vm.get(&0).is_none();

    let mut oc_vm = trx.oc_vm_mut().await?;
    let _ = oc_vm.get(&0).is_none();
    oc_vm.insert(1, User::default()).await?;
    oc_vm.remove(2).await?;

    // once_cell<Version<VecMap<_>>>
    let oc_v_vm = trx.oc_v_vm().await?;
    let _ = oc_v_vm.get(&0).is_none();

    let mut oc_v_vm = trx.oc_vm_mut().await?;
    let _ = oc_v_vm.get(&0).is_none();
    oc_v_vm.insert(1, User::default()).await?;
    oc_v_vm.remove(2).await?;

    // Cache<_>
    let c = trx.c();
    let _ = c.get(&0).is_none();

    let mut c = trx.c_mut();
    let _ = c.get(&0).is_none();
    c.insert(1, User::default()).await?;
    c.remove(2).await?;

    // Version<Cache<_>>
    let v_c = trx.v_c();
    let _ = v_c.get(&0).is_none();

    let mut v_c = trx.v_c_mut();
    let _ = v_c.get(&0).is_none();
    v_c.insert(1, User::default()).await?;
    v_c.remove(2).await?;

    // tests that all traits are correctly propagating.
    async fn actions_mut<T>(mut t: T) -> Result<()>
    where
        T: Get<User> + Insert<User> + Remove<User>,
    {
        let _ = t.get(&0).is_none();
        t.insert(1, User::default()).await?;
        t.remove(2).await?;
        Ok(())
    }

    actions_mut(trx.oc_vm_mut().await?).await?;
    actions_mut(trx.oc_v_vm_mut().await?).await?;
    actions_mut(trx.c_mut()).await?;
    actions_mut(trx.v_c_mut()).await?;

    Ok(())
}

#[derive(Ctx, Default)]
struct Ctx {
    oc_vm: OnceCell<VecMap<usize, User>>,
    oc_v_vm: OnceCell<Version<VecMap<usize, User>>>,

    c: Cache<usize, User>,
    v_c: Version<Cache<usize, User>>,
}

#[derive(Default)]
struct User {
    pub name: String,
}

impl Entity for User {
    type Key = usize;
}
