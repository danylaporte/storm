use storm::{prelude::*, BoxFuture, EntityAsset, HashOneMany, NoopLoad, Result, Trx};

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(Default::default())
}

#[tokio::test]
async fn create_async() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let ctx = create_ctx();
        let ctx = ctx.read().await?;
        let _id: &usize = ctx.ref_as::<NextId>().await?;
        Ok(())
    })
    .await
}

#[derive(Ctx, Default, NoopLoad, PartialEq)]
struct User {
    #[allow(dead_code)]
    pub name: String,
}

impl Entity for User {
    type Key = usize;
    type TrackCtx = ();
}

#[indexing]
fn next_id(tbl: &Users) -> usize {
    tbl.iter().map(|t| *t.0).max().unwrap_or_default()
}

#[indexing]
fn next_id2(_tbl: &Users, next_id: &NextId) -> usize {
    **next_id
}

#[indexing]
fn index_with_ctx(_ctx: &Ctx, tbl: &Users) -> usize {
    tbl.len()
}

#[storm_derive::index]
async fn index_id_by_name(ctx: &Ctx) -> Result<HashOneMany<String, usize, Self>> {
    User::changed().register(&user_changed);

    let user = ctx.tbl_of::<User>().await?;

    Ok(user.iter().map(|(id, u)| (u.name.clone(), *id)).collect())
}

fn user_changed(
    trx: &mut Trx<'_>,
    id: &usize,
    user: &User,
    track: &(),
) -> BoxFuture<'static, Result<()>> {
    Box::pin(async move {
        if let Some(index) = trx.asset_opt::<IndexIdByName>() {
            index.insert(user.name.clone(), *id);
        }
    })
}
