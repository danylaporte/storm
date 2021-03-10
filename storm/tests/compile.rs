use async_cell_lock::QueueRwLock;
use cache::Cache;
use storm::{prelude::*, AsyncOnceCell, Connected, Ctx, Entity, GetVersion, Result};
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
    let oc_vms = ctx.oc_vms().await?;
    let _ = oc_vms.get(&0).is_none();
    let _ = oc_vms.iter();

    // once_cell<Version<VecMap<_>>>
    let oc_v_vms = ctx.oc_v_vms().await?;
    let _ = oc_v_vms.get(&0).is_none();
    let _ = oc_v_vms.get_version();
    let _ = oc_v_vms.iter();

    // Cache<_>
    let cs = ctx.cs();
    let _ = cs.get(&0).is_none();
    let _ = cs.iter();

    // Version<Cache<_>>
    let v_cs = ctx.v_cs();
    let _ = v_cs.get(&0).is_none();
    let _ = v_cs.get_version();
    let _ = v_cs.iter();

    Ok(())
}

#[tokio::test]
async fn transaction() -> Result<()> {
    let ctx = create_ctx();

    let ctx = ctx.read().await.queue().await;

    let mut trx = ctx.transaction().await?;

    // once_cell<VecMap<_>>
    let oc_vms = trx.oc_vms().await?;
    let _ = oc_vms.get(&0).is_none();

    let mut oc_vms = trx.oc_vms_mut().await?;
    let _ = oc_vms.get(&0).is_none();
    oc_vms.insert(1, OcVm::default()).await?;
    oc_vms.remove(2).await?;

    // once_cell<Version<VecMap<_>>>
    let oc_v_vms = trx.oc_v_vms().await?;
    let _ = oc_v_vms.get(&0).is_none();

    let mut oc_v_vms = trx.oc_v_vms_mut().await?;
    let _ = oc_v_vms.get(&0).is_none();
    oc_v_vms.insert(1, OcVVm::default()).await?;
    oc_v_vms.remove(2).await?;

    // Cache<_>
    let cs = trx.cs();
    let _ = cs.get(&0).is_none();

    let mut cs = trx.cs_mut();
    let _ = cs.get(&0).is_none();
    cs.insert(1, C::default()).await?;
    cs.remove(2).await?;

    // Version<Cache<_>>
    let v_cs = trx.v_cs();
    let _ = v_cs.get(&0).is_none();

    let mut v_cs = trx.v_cs_mut();
    let _ = v_cs.get(&0).is_none();
    v_cs.insert(1, VC::default()).await?;
    v_cs.remove(2).await?;

    // tests that all traits are correctly propagating.
    async fn actions_mut<T, E>(mut t: T) -> Result<()>
    where
        E: Default + Entity<Key = usize>,
        T: Get<E> + Insert<E> + Remove<E>,
    {
        let _ = t.get(&0).is_none();
        t.insert(1, E::default()).await?;
        t.remove(2).await?;
        Ok(())
    }

    actions_mut(trx.oc_vms_mut().await?).await?;
    actions_mut(trx.oc_v_vms_mut().await?).await?;
    actions_mut(trx.cs_mut()).await?;
    actions_mut(trx.v_cs_mut()).await?;

    Ok(())
}

#[derive(Ctx, Default)]
struct Ctx {
    oc_vms: AsyncOnceCell<VecMap<usize, OcVm>>,
    oc_v_vms: AsyncOnceCell<Version<VecMap<usize, OcVVm>>>,
    cs: Cache<usize, C>,
    v_cs: Version<Cache<usize, VC>>,
}

macro_rules! entity {
    ($n:ident) => {
        #[derive(Default)]
        struct $n {
            pub name: String,
        }

        impl Entity for $n {
            type Key = usize;
        }
    };
}

entity!(OcVm);
entity!(OcVVm);
entity!(C);
entity!(VC);
