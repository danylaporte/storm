use storm::{prelude::*, NoopDelete, NoopLoad, NoopSave, Result};

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(Default::default())
}

#[tokio::test]
async fn flow() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let ctx = create_ctx();

        let ctx = ctx.read().await?;
        let ctx = ctx.queue().await?;

        let trx = ctx.transaction();
        let log = trx.commit().await?;

        let mut ctx = ctx.write().await?;

        ctx.apply_log(log);

        let _ = ctx.read().await?;

        Ok(())
    })
    .await
}

#[tokio::test]
async fn read() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let ctx = create_ctx();

        let ctx = ctx.read().await?;

        let entity1s = ctx.tbl_of::<Entity1>().await?;
        let _ = entity1s.get(&0).is_none();
        let _ = entity1s.iter();

        let entity2s = ctx.tbl_of::<Entity2>().await?;
        let _ = entity2s.get(&0).is_none();
        let _ = entity2s.iter();

        Ok(())
    })
    .await
}

#[tokio::test]
async fn transaction() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let ctx = create_ctx();

        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction();

        let mut entity1s = trx.tbl_of::<Entity1>().await?;
        let _ = entity1s.get(&0).is_none();
        entity1s.insert(1, Entity1::default()).await?;
        entity1s.remove(2).await?;

        let mut entity2s = trx.tbl_of::<Entity2>().await?;
        let _ = entity2s.get(&0).is_none();
        entity2s.insert(1, Entity2::default()).await?;
        entity2s.remove(2).await?;

        Ok(())
    })
    .await
}

macro_rules! entity {
    ($n:ident) => {
        #[derive(Ctx, Default, NoopDelete, NoopLoad, NoopSave)]
        struct $n {
            pub name: String,
        }

        impl Entity for $n {
            type Key = usize;
        }
    };
}

entity!(Entity1);
entity!(Entity2);
