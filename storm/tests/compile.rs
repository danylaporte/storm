use storm::{prelude::*, NoopDelete, NoopLoad, NoopSave, Result};
use uuid::Uuid;

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(Default::default(), "ctx")
}

#[tokio::test]
async fn flow() -> Result<()> {
    async_cell_lock::with_deadlock_check(
        async move {
            let ctx = create_ctx();

            let ctx = ctx.read().await?;
            let ctx = ctx.queue().await?;

            let trx = ctx.transaction(Uuid::nil());
            let log = trx.commit().await?;

            let mut ctx = ctx.write().await?;

            ctx.apply_log(log);

            let _ = ctx.read().await?;

            Ok(())
        },
        "compile",
    )
    .await
}

#[tokio::test]
async fn read() -> Result<()> {
    async_cell_lock::with_deadlock_check(
        async move {
            let ctx = create_ctx();

            let ctx = ctx.read().await?;

            let entity1s = ctx.tbl_of::<Entity1>().await?;
            let _ = entity1s.get(&0).is_none();
            let _ = entity1s.iter();

            let entity2s = ctx.tbl_of::<Entity2>().await?;
            let _ = entity2s.get(&0).is_none();
            let _ = entity2s.iter();

            Ok(())
        },
        "read",
    )
    .await
}

#[tokio::test]
async fn transaction() -> Result<()> {
    async_cell_lock::with_deadlock_check(
        async move {
            let ctx = create_ctx();
            let ctx = ctx.queue().await?;
            let mut trx = ctx.transaction(Uuid::nil());

            let mut entity1s = trx.tbl_of::<Entity1>().await?;
            let _ = entity1s.get(&0).is_none();
            entity1s.insert(1, Entity1::default()).await?;
            entity1s.remove(2).await?;

            let mut entity2s = trx.tbl_of::<Entity2>().await?;
            let _ = entity2s.get(&0).is_none();
            entity2s.insert(1, Entity2::default()).await?;
            entity2s.remove(2).await?;

            Ok(())
        },
        "transaction",
    )
    .await
}

#[derive(storm::LocksAwait)]
struct Locks<'a> {
    e1: &'a Entity1s,
    e2: &'a Entity2s,
    e3: &'a Entity3s,
    e4: &'a Entity4s,
    e5: &'a Entity5s,
}

macro_rules! entity {
    ($n:ident) => {
        #[derive(Ctx, Default, NoopDelete, NoopLoad, NoopSave, PartialEq)]
        struct $n {
            #[allow(dead_code)]
            pub name: String,
        }

        impl Entity for $n {
            type Key = u32;
        }
    };
}

entity!(Entity1);
entity!(Entity2);
entity!(Entity3);
entity!(Entity4);
entity!(Entity5);
