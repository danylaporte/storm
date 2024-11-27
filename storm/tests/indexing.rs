use storm::{prelude::*, BoxFuture, EntityObj, HashOneMany, NoopLoad, Result, Trx};

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(Default::default())
}

#[tokio::test]
async fn create_async() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let ctx = create_ctx();
        let ctx = ctx.read().await?;
        //let _id: &usize = ctx.ref_as::<NextId>().await?;

        let _index = ctx.obj::<IndexIdByName>().await?;

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

// #[indexing]
// fn next_id(tbl: &Users) -> usize {
//     tbl.iter().map(|t| *t.0).max().unwrap_or_default()
// }

// #[indexing]
// fn next_id2(_tbl: &Users, next_id: &NextId) -> usize {
//     **next_id
// }

// #[indexing]
// fn index_with_ctx(_ctx: &Ctx, tbl: &Users) -> usize {
//     tbl.len()
// }

#[storm_derive::index]
async fn index_id_by_name(ctx: &Ctx) -> Result<HashOneMany<String, usize>> {
    User::changed().register(&user_changed);
    Ctx::on_clear_obj::<Users>().register_clear_obj::<Self>();

    let user = ctx.tbl_of::<User>().await?;

    let idx = user.iter().map(|(id, u)| (u.name.clone(), *id)).collect();

    Ok(idx)
}

fn user_changed<'a>(
    trx: &'a mut Trx<'_>,
    id: &'a usize,
    user: &'a User,
    _track: &'a (),
) -> BoxFuture<'a, Result<()>> {
    Box::pin(async move {
        if let Some(mut index) = trx.obj_opt::<IndexIdByName>() {
            index.insert(user.name.clone(), *id);
        }

        Ok(())
    })
}
