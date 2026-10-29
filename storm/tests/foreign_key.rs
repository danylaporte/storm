use storm::{Error, NoopDelete, NoopLoad, NoopSave, Result, prelude::*};
use uuid::Uuid;

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(Default::default(), "ctx")
}

#[async_cell_lock::tokio_test]
async fn insert_requires_existing_parent() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;

    {
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx.insert(10, RChild { parent_id: 1 }).await.unwrap_err();

        assert!(
            matches!(
                err,
                Error::ForeignKeyNotFound {
                    entity: "RChild",
                    field: "parent_id",
                    parent: "RParent",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(1, RParent).await?;
    trx.insert(10, RChild { parent_id: 1 }).await?;

    assert!(trx.index::<RChildParentIdFkIndex>().await?.contains(1, 10));

    trx.commit().await?.apply_log(ctx).await?;
    Ok(())
}

#[async_cell_lock::tokio_test]
async fn restrict_blocks_parent_removal() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, RParent).await?;
        trx.insert(2, RParent).await?;
        trx.insert(10, RChild { parent_id: 1 }).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.queue().await?;

    {
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx.remove::<RParent>(1).await.unwrap_err();

        assert!(
            matches!(err, Error::ForeignKeyRestricted { count: 1, .. }),
            "{err:?}"
        );
    }

    {
        // a child added in the transaction also blocks.
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(11, RChild { parent_id: 2 }).await?;
        assert!(trx.remove::<RParent>(2).await.is_err());
    }

    {
        // moving the child to another parent releases the restriction.
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(10, RChild { parent_id: 2 }).await?;
        assert!(trx.remove::<RParent>(1).await?);
        assert!(trx.remove::<RParent>(2).await.is_err());
    }

    let mut trx = ctx.transaction(Uuid::nil());
    trx.remove::<RChild>(10).await?;
    assert!(trx.remove::<RParent>(1).await?);
    trx.commit().await?.apply_log(ctx).await?;

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn cascade_removes_children_recursively() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, CParent).await?;
        trx.insert(2, CParent).await?;
        trx.insert(10, CChild { parent_id: Some(1) }).await?;
        trx.insert(11, CChild { parent_id: Some(1) }).await?;
        trx.insert(12, CChild { parent_id: Some(2) }).await?;
        trx.insert(13, CChild { parent_id: None }).await?;
        trx.insert(100, CGrandChild { child_id: 10 }).await?;
        trx.insert(101, CGrandChild { child_id: 12 }).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());

        // not committed yet, must also be cascaded.
        trx.insert(14, CChild { parent_id: Some(1) }).await?;

        assert!(trx.remove::<CParent>(1).await?);

        let children = trx.tbl_of::<CChild>().await?;
        assert!(children.get(&10).is_none());
        assert!(children.get(&11).is_none());
        assert!(children.get(&12).is_some());
        assert!(children.get(&13).is_some());
        assert!(children.get(&14).is_none());

        let grand_children = trx.tbl_of::<CGrandChild>().await?;
        assert!(grand_children.get(&100).is_none());
        assert!(grand_children.get(&101).is_some());

        assert!(
            trx.index::<CChildParentIdFkIndex>()
                .await?
                .get(1)
                .is_empty()
        );
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.read().await?;
    let children = ctx.tbl_of::<CChild>().await?;
    let mut keys = children.keys().copied().collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, vec![12, 13]);

    let grand_children = ctx.tbl_of::<CGrandChild>().await?;
    assert_eq!(
        grand_children.keys().copied().collect::<Vec<_>>(),
        vec![101]
    );

    let idx = ctx.ref_as::<CChildParentIdFkIndex>().await?;
    assert!(idx.get(1).is_empty());
    assert!(idx.contains(2, 12));

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn set_null_clears_children() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, NParent).await?;
        trx.insert(2, NParent).await?;
        trx.insert(
            10,
            NChild {
                parent_id: Some(1),
                name: "a".into(),
            },
        )
        .await?;
        trx.insert(
            11,
            NChild {
                parent_id: Some(2),
                name: "b".into(),
            },
        )
        .await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        assert!(trx.remove::<NParent>(1).await?);

        let children = trx.tbl_of::<NChild>().await?;
        assert_eq!(
            children.get(&10),
            Some(&NChild {
                parent_id: None,
                name: "a".into()
            })
        );
        assert_eq!(children.get(&11).and_then(|c| c.parent_id), Some(2));

        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.read().await?;
    let children = ctx.tbl_of::<NChild>().await?;
    assert_eq!(children.get(&10).and_then(|c| c.parent_id), None);
    assert_eq!(children.get(&11).and_then(|c| c.parent_id), Some(2));
    assert!(
        ctx.ref_as::<NChildIdsByParentId>()
            .await?
            .get(1)
            .is_empty()
    );

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn self_reference_cascade() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());

    trx.insert(1, Node { parent: None }).await?;
    trx.insert(2, Node { parent: Some(1) }).await?;
    trx.insert(3, Node { parent: Some(2) }).await?;
    trx.insert(4, Node { parent: None }).await?;

    // a row may reference itself.
    trx.insert(5, Node { parent: Some(5) }).await?;

    assert!(trx.insert(6, Node { parent: Some(99) }).await.is_err());

    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(1, Node { parent: None }).await?;
    trx.insert(2, Node { parent: Some(1) }).await?;
    trx.insert(3, Node { parent: Some(2) }).await?;
    trx.insert(4, Node { parent: None }).await?;
    trx.insert(5, Node { parent: Some(5) }).await?;

    assert!(trx.remove::<Node>(1).await?);
    assert!(trx.remove::<Node>(5).await?);

    let nodes = trx.tbl_of::<Node>().await?;
    assert_eq!(nodes.keys().copied().collect::<Vec<_>>(), vec![4]);

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn self_reference_restrict() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());

    trx.insert(1, RNode { parent: Some(1) }).await?;
    trx.insert(2, RNode { parent: Some(1) }).await?;
    trx.insert(3, RNode { parent: Some(3) }).await?;

    // the reference to itself does not count.
    assert!(trx.plan_remove::<RNode>(1).await?.is_restricted());
    assert!(!trx.plan_remove::<RNode>(3).await?.is_restricted());
    assert!(trx.remove::<RNode>(3).await?);

    let err = trx.remove::<RNode>(1).await.unwrap_err();
    assert!(
        matches!(err, Error::ForeignKeyRestricted { count: 1, .. }),
        "{err:?}"
    );

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn delete_plan_in_batches() -> Result<()> {
    use storm::DeleteAction;

    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, CParent).await?;
        trx.insert(2, CParent).await?;
        trx.insert(10, CChild { parent_id: Some(1) }).await?;
        trx.insert(11, CChild { parent_id: Some(1) }).await?;
        trx.insert(12, CChild { parent_id: Some(2) }).await?;
        trx.insert(100, CGrandChild { child_id: 10 }).await?;
        trx.insert(101, CGrandChild { child_id: 10 }).await?;
        trx.insert(102, CGrandChild { child_id: 11 }).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let mut plan = {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.plan_remove::<CParent>(1).await?
    };

    // the dry run does not change anything.
    assert_eq!(
        ctx.read().await?.tbl_of::<CChild>().await?.keys().count(),
        3
    );

    assert_eq!(plan.len(), 6);
    assert!(!plan.is_restricted());
    assert_eq!(
        plan.summary(),
        vec![
            ("CParent", DeleteAction::Remove, 1),
            ("CChild", DeleteAction::Remove, 2),
            ("CGrandChild", DeleteAction::Remove, 3),
        ]
    );

    let root = plan.nodes().next().expect("root");
    assert_eq!(root.key_as::<u32>(), Some(&1));
    assert_eq!(root.parent, None);

    for n in plan.nodes().skip(1) {
        let parent = plan.nodes().nth(n.parent.expect("parent")).expect("node");
        assert_eq!(n.depth, parent.depth + 1);
    }

    let mut batches = 0;

    while !plan.is_done() {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());

        if batches == 1 {
            // moved to another parent between two batches: no longer cascaded.
            trx.insert(11, CChild { parent_id: Some(2) }).await?;
        }

        assert!(plan.execute(&mut trx, 2).await? <= 2);
        trx.commit().await?.apply_log(ctx).await?;
        batches += 1;
    }

    assert_eq!(batches, 3);
    assert_eq!(plan.remaining(), 0);

    let ctx = ctx.read().await?;
    assert!(ctx.tbl_of::<CParent>().await?.get(&1).is_none());

    let mut keys = ctx
        .tbl_of::<CChild>()
        .await?
        .keys()
        .copied()
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, vec![11, 12]);

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn delete_plan_restricted_and_set_null() -> Result<()> {
    use storm::DeleteAction;

    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(1, RParent).await?;
    trx.insert(10, RChild { parent_id: 1 }).await?;
    trx.insert(2, NParent).await?;
    trx.insert(
        20,
        NChild {
            parent_id: Some(2),
            name: "a".into(),
        },
    )
    .await?;

    let mut plan = trx.plan_remove::<RParent>(1).await?;
    assert!(plan.is_restricted());
    assert!(matches!(
        plan.restrictions()[0],
        storm::delete_plan::DeleteRestriction::ForeignKey { count: 1, .. }
    ));
    assert!(matches!(
        plan.execute(&mut trx, 10).await,
        Err(Error::ForeignKeyRestricted { .. })
    ));
    assert!(trx.tbl_of::<RParent>().await?.contains(&1));

    let mut plan = trx.plan_remove::<NParent>(2).await?;
    assert_eq!(
        plan.summary(),
        vec![
            ("NParent", DeleteAction::Remove, 1),
            ("NChild", DeleteAction::SetNull, 1),
        ]
    );
    assert_eq!(plan.execute(&mut trx, 10).await?, 2);
    assert_eq!(
        trx.tbl_of::<NChild>()
            .await?
            .get(&20)
            .and_then(|c| c.parent_id),
        None
    );

    assert!(trx.plan_remove::<NParent>(2).await?.is_empty());
    Ok(())
}

#[async_cell_lock::tokio_test]
async fn remove_rules() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(1, BParent).await?;
    trx.insert(
        10,
        BChild {
            parent_id: 1,
            locked: false,
        },
    )
    .await?;
    trx.insert(
        11,
        BChild {
            parent_id: 1,
            locked: true,
        },
    )
    .await?;

    let err = trx.remove::<BChild>(11).await.unwrap_err();
    assert!(
        matches!(&err, Error::RemoveRestricted { entity: "BChild", reason, .. } if reason == "locked"),
        "{err:?}"
    );

    // a blocked removal does not poison the transaction.
    assert!(trx.remove::<BChild>(10).await?);
    trx.insert(
        10,
        BChild {
            parent_id: 1,
            locked: false,
        },
    )
    .await?;

    let mut plan = trx.plan_remove::<BParent>(1).await?;
    let [restriction] = plan.restrictions() else {
        panic!("{:?}", plan.restrictions());
    };
    assert!(matches!(
        restriction,
        storm::delete_plan::DeleteRestriction::Rule { .. }
    ));
    let node = plan.nodes().nth(restriction.node()).expect("node");
    assert_eq!(node.key_as::<u32>(), Some(&11));
    assert!(matches!(
        plan.execute(&mut trx, 10).await,
        Err(Error::RemoveRestricted { .. })
    ));
    assert_eq!(trx.tbl_of::<BChild>().await?.keys().count(), 2);

    trx.insert(
        11,
        BChild {
            parent_id: 1,
            locked: false,
        },
    )
    .await?;
    let mut plan = trx.plan_remove::<BParent>(1).await?;
    assert!(!plan.is_restricted());
    assert_eq!(plan.execute(&mut trx, 10).await?, 3);
    assert_eq!(trx.tbl_of::<BChild>().await?.keys().count(), 0);

    Ok(())
}

#[test]
fn metadata() {
    use storm::{ForeignKey, OnDelete};

    assert_eq!(RChildParentIdFk::ON_DELETE, OnDelete::Restrict);
    assert_eq!(CChildParentIdFk::ON_DELETE, OnDelete::Cascade);
    assert_eq!(NChildParentIdFk::ON_DELETE, OnDelete::SetNull);
    assert_eq!(CGrandChildChildIdFk::FIELD, "child_id");
}

// restrict (default)

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct RParent;

impl Entity for RParent {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct RChild {
    #[storm(foreign_key(entity = RParent))]
    parent_id: u32,
}

impl Entity for RChild {
    type Key = u32;
}

// cascade

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct CParent;

impl Entity for CParent {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(collection = "hash_table")]
struct CChild {
    #[storm(foreign_key(entity = CParent, on_delete = "cascade"))]
    parent_id: Option<u32>,
}

impl Entity for CChild {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct CGrandChild {
    #[storm(foreign_key(entity = CChild, on_delete = "cascade"))]
    child_id: u32,
}

impl Entity for CGrandChild {
    type Key = u32;
}

// set null

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct NParent;

impl Entity for NParent {
    type Key = u32;
}

#[derive(Clone, Ctx, Debug, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct NChild {
    #[storm(foreign_key(entity = NParent, on_delete = "set_null", index = "NChildIdsByParentId"))]
    parent_id: Option<u32>,
    name: String,
}

impl Entity for NChild {
    type Key = u32;
}

// flat index

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct FParent;

impl Entity for FParent {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct FChild {
    #[storm(foreign_key(entity = FParent, on_delete = "cascade", flat_index = "FChildIdsByParentId"))]
    parent_id: Option<u32>,
}

impl Entity for FChild {
    type Key = u32;
}

#[async_cell_lock::tokio_test]
async fn flat_index_cascade() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, FParent).await?;
        trx.insert(2, FParent).await?;
        trx.insert(10, FChild { parent_id: Some(1) }).await?;
        trx.insert(11, FChild { parent_id: Some(2) }).await?;
        trx.insert(12, FChild { parent_id: None }).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(13, FChild { parent_id: Some(1) }).await?;
    assert!(trx.remove::<FParent>(1).await?);

    let idx = trx.index::<FChildIdsByParentId>().await?;
    assert!(idx.get(1).is_empty());
    assert!(idx.contains(2, 11));
    assert!(idx.contains_none(12));

    let children = trx.tbl_of::<FChild>().await?;
    let mut keys = children.keys().copied().collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, vec![11, 12]);

    Ok(())
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct HashChild {
    #[storm(foreign_key(entity = FParent, hash))]
    parent_id: u32,
}

impl Entity for HashChild {
    type Key = u32;
}

#[async_cell_lock::tokio_test]
async fn hash_flag_uses_hash_index() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(1, FParent).await?;
    trx.insert(2, FParent).await?;
    trx.insert(10, HashChild { parent_id: 1 }).await?;

    // not declared with a name: found with a scan, the index is not built.
    assert!(trx.plan_remove::<FParent>(1).await?.is_restricted());
    assert!(
        ctx.ctx_ext_obj()
            .get(<HashChildParentIdFkIndexAdapt as storm::indexing::HashFlatSetAdapt>::index_var())
            .get()
            .is_none()
    );

    trx.insert(11, HashChild { parent_id: 2 }).await?;

    let idx = trx.index::<HashChildParentIdFkIndex>().await?;
    assert!(idx.contains(&1, 10));
    assert!(idx.contains(&2, 11));

    assert!(trx.plan_remove::<FParent>(2).await?.is_restricted());

    Ok(())
}

// self reference

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct Node {
    #[storm(foreign_key(entity = Node, on_delete = "cascade"))]
    parent: Option<u32>,
}

impl Entity for Node {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct RNode {
    #[storm(foreign_key(entity = RNode))]
    parent: Option<u32>,
}

impl Entity for RNode {
    type Key = u32;
}

// business rules

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct BParent;

impl Entity for BParent {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct BChild {
    #[storm(foreign_key(entity = BParent, on_delete = "cascade"))]
    parent_id: u32,
    locked: bool,
}

impl Entity for BChild {
    type Key = u32;
}

#[storm::register]
fn register_rules() {
    storm::on_remove_rule::<BChild>(child_not_locked);
    <HParent as storm::EntityAccessor>::removing().on(remove_h_children);
}

/// Detaches the children before the restriction is checked.
fn remove_h_children<'a>(
    trx: &'a mut storm::CtxTransaction<'_>,
    key: &'a u32,
) -> storm::BoxFuture<'a, Result<()>> {
    Box::pin(async move {
        let keys = storm::foreign_key::children_of::<HChildParentIdFk>(trx, key).await?;

        for k in keys {
            trx.remove::<HChild>(k).await?;
        }

        Ok(())
    })
}

#[async_cell_lock::tokio_test]
async fn restrict_checked_after_removing_handlers() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(1, HParent).await?;
    trx.insert(10, HChild { parent_id: 1 }).await?;
    trx.insert(11, HChild { parent_id: 1 }).await?;

    assert!(trx.remove::<HParent>(1).await?);
    assert_eq!(trx.tbl_of::<HChild>().await?.keys().count(), 0);

    // found with a scan: the reverse index is not built.
    assert!(
        ctx.ctx_ext_obj()
            .get(<HChildParentIdFkIndexAdapt as storm::indexing::FlatSetAdapt>::index_var())
            .get()
            .is_none()
    );

    Ok(())
}

// restrict, detached by a removing handler

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct HParent;

impl Entity for HParent {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct HChild {
    #[storm(foreign_key(entity = HParent))]
    parent_id: u32,
}

impl Entity for HChild {
    type Key = u32;
}

fn child_not_locked<'a>(
    trx: &'a mut storm::CtxTransaction<'_>,
    key: &'a u32,
) -> storm::BoxFuture<'a, Result<Option<String>>> {
    Box::pin(async move {
        let locked = trx
            .get_entity::<BChild>(key)
            .await?
            .is_some_and(|c| c.locked);
        Ok(locked.then(|| "locked".to_owned()))
    })
}

// foreign keys on key columns

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct KParent;

impl Entity for KParent {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(foreign_key(column = "Id", entity = KParent, on_delete = "cascade"))]
struct KChild;

impl Entity for KChild {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(
    collection = "hash_table",
    foreign_key(column = "ParentId", entity = KParent, key = 0),
    foreign_key(column = "OtherId", entity = RParent, on_delete = "cascade", key = 1)
)]
struct KLink;

impl Entity for KLink {
    type Key = (u32, u32);
}

#[test]
fn key_metadata() {
    use storm::{ForeignKey, OnDelete};

    assert_eq!(KChildIdFk::FIELD, "Id");
    assert_eq!(KChildIdFk::ON_DELETE, OnDelete::Cascade);
    assert_eq!(KLinkParentIdFk::ON_DELETE, OnDelete::Restrict);
    assert_eq!(KLinkOtherIdFk::FIELD, "OtherId");
}

#[async_cell_lock::tokio_test]
async fn key_insert_requires_existing_parent() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;

    {
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx.insert(1, KChild).await.unwrap_err();

        assert!(
            matches!(
                err,
                Error::ForeignKeyNotFound {
                    entity: "KChild",
                    field: "Id",
                    parent: "KParent",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    {
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, KParent).await?;
        let err = trx.insert((1, 5), KLink).await.unwrap_err();

        assert!(
            matches!(
                err,
                Error::ForeignKeyNotFound {
                    field: "OtherId",
                    parent: "RParent",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(1, KParent).await?;
    trx.insert(1, KChild).await?;
    trx.insert(5, RParent).await?;
    trx.insert((1, 5), KLink).await?;

    trx.commit().await?.apply_log(ctx).await?;
    Ok(())
}

#[async_cell_lock::tokio_test]
async fn key_cascade_and_restrict() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, KParent).await?;
        trx.insert(2, KParent).await?;
        trx.insert(1, KChild).await?;
        trx.insert(2, KChild).await?;
        trx.insert(5, RParent).await?;
        trx.insert((1, 5), KLink).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.queue().await?;

    {
        // the composite key child restricts the removal (found with a scan).
        let mut trx = ctx.transaction(Uuid::nil());
        assert!(trx.plan_remove::<KParent>(1).await?.is_restricted());

        let err = trx.remove::<KParent>(1).await.unwrap_err();
        assert!(
            matches!(
                err,
                Error::ForeignKeyRestricted {
                    entity: "KLink",
                    field: "ParentId",
                    count: 1,
                    ..
                }
            ),
            "{err:?}"
        );
    }

    let mut trx = ctx.transaction(Uuid::nil());

    // cascades on the second column of the composite key.
    assert!(trx.remove::<RParent>(5).await?);
    assert_eq!(trx.tbl_of::<KLink>().await?.keys().count(), 0);

    // cascades on the single key.
    assert!(!trx.plan_remove::<KParent>(1).await?.is_restricted());
    assert!(trx.remove::<KParent>(1).await?);

    let children = trx.tbl_of::<KChild>().await?;
    assert!(children.get(&1).is_none());
    assert!(children.get(&2).is_some());

    trx.commit().await?.apply_log(ctx).await?;
    Ok(())
}

// field foreign key without reverse index, on a composite key child

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(collection = "hash_table")]
struct NoIndexLink {
    #[storm(foreign_key(entity = RParent, no_index))]
    owner_id: u32,
}

impl Entity for NoIndexLink {
    type Key = (u32, u32);
}

#[async_cell_lock::tokio_test]
async fn no_index_on_composite_key_child() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx
            .insert((1, 2), NoIndexLink { owner_id: 7 })
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::ForeignKeyNotFound {
                    entity: "NoIndexLink",
                    field: "owner_id",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(7, RParent).await?;
        trx.insert((1, 2), NoIndexLink { owner_id: 7 }).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.queue().await?;

    {
        let mut trx = ctx.transaction(Uuid::nil());
        assert!(trx.plan_remove::<RParent>(7).await?.is_restricted());
        assert!(matches!(
            trx.remove::<RParent>(7).await.unwrap_err(),
            Error::ForeignKeyRestricted { count: 1, .. }
        ));
    }

    let mut trx = ctx.transaction(Uuid::nil());
    trx.remove::<NoIndexLink>((1, 2)).await?;
    assert!(trx.remove::<RParent>(7).await?);

    Ok(())
}

// external foreign keys: declared with `table` on the child, registered by the parent with
// `referenced_by` (the parent can be in a crate depending on the child crate).

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(
    referenced_by = XChildParentIdFk,
    referenced_by = XNullChildParentIdFk,
    referenced_by = XLinkOwnerIdFk
)]
struct XParent;

impl Entity for XParent {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct XChild {
    #[storm(foreign_key(table = "[dbo].[XParents]", on_delete = "cascade", index = "XChildIdsByParentId"))]
    parent_id: u32,
}

impl Entity for XChild {
    type Key = u32;
}

#[derive(Clone, Ctx, Debug, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct XNullChild {
    #[storm(foreign_key(table = "[dbo].[XParents]", on_delete = "set_null"))]
    parent_id: Option<u32>,
}

impl Entity for XNullChild {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(collection = "hash_table")]
struct XLink {
    #[storm(foreign_key(table = "[dbo].[XParents]", no_index))]
    owner_id: u32,
}

impl Entity for XLink {
    type Key = (u32, u32);
}

#[test]
fn external_metadata() {
    use storm::{OnDelete, foreign_key::ExternalForeignKey};

    assert_eq!(XChildParentIdFk::ON_DELETE, OnDelete::Cascade);
    assert_eq!(XNullChildParentIdFk::ON_DELETE, OnDelete::SetNull);
    assert_eq!(XLinkOwnerIdFk::ON_DELETE, OnDelete::Restrict);
    assert_eq!(XChildParentIdFk::FIELD, "parent_id");
}

#[async_cell_lock::tokio_test]
async fn external_foreign_keys() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx.insert(10, XChild { parent_id: 1 }).await.unwrap_err();

        assert!(
            matches!(
                err,
                Error::ForeignKeyNotFound {
                    entity: "XChild",
                    field: "parent_id",
                    parent: "XParent",
                    ..
                }
            ),
            "{err:?}"
        );
    }

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, XParent).await?;
        trx.insert(2, XParent).await?;
        trx.insert(10, XChild { parent_id: 1 }).await?;
        trx.insert(20, XNullChild { parent_id: Some(1) }).await?;
        trx.insert((5, 6), XLink { owner_id: 2 }).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.queue().await?;

    // restrict.
    {
        let mut trx = ctx.transaction(Uuid::nil());
        assert!(trx.plan_remove::<XParent>(2).await?.is_restricted());
        assert!(matches!(
            trx.remove::<XParent>(2).await.unwrap_err(),
            Error::ForeignKeyRestricted {
                entity: "XLink",
                parent: "XParent",
                count: 1,
                ..
            }
        ));
    }

    let mut trx = ctx.transaction(Uuid::nil());
    assert!(trx.index::<XChildIdsByParentId>().await?.contains(1, 10));

    // cascade and set null.
    assert!(trx.remove::<XParent>(1).await?);
    assert!(trx.tbl_of::<XChild>().await?.get(&10).is_none());
    assert_eq!(
        trx.tbl_of::<XNullChild>().await?.get(&20).map(|c| c.parent_id),
        Some(None)
    );

    trx.remove::<XLink>((5, 6)).await?;
    assert!(trx.remove::<XParent>(2).await?);

    Ok(())
}

// schema_only: declared for the schema validation only, not enforced (ex: entity not saved by storm).

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct SoParent;

impl Entity for SoParent {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, PartialEq)]
struct SoChild {
    #[storm(foreign_key(entity = SoParent, schema_only))]
    parent_id: u32,

    #[storm(foreign_key(table = "[dbo].[Files]", on_delete = "set_null", schema_only))]
    file_id: Option<u32>,

    // the type is not used: a wrapper of an `Option` is accepted.
    #[storm(foreign_key(table = "[dbo].[Imports]", on_delete = "set_null", schema_only))]
    import_id: Wrapper<Option<u32>>,
}

#[derive(PartialEq)]
struct Wrapper<T>(T);

impl<T> storm::Gc for Wrapper<T> {}

impl Entity for SoChild {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(
    foreign_key(column = "FileId", table = "[dbo].[Files]", schema_only),
    foreign_key(column = "OtherId", entity = SoParent, on_delete = "cascade", schema_only)
)]
struct SoSavedChild {
    #[storm(foreign_key(entity = SoParent, schema_only))]
    parent_id: u32,
}

impl Entity for SoSavedChild {
    type Key = u32;
}

#[async_cell_lock::tokio_test]
async fn schema_only_is_not_enforced() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, SoParent).await?;
        trx.insert(10, SoSavedChild { parent_id: 1 }).await?;
        trx.insert(11, SoSavedChild { parent_id: 99 }).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());
    assert!(trx.remove::<SoParent>(1).await?);
    assert!(trx.tbl_of::<SoSavedChild>().await?.get(&10).is_some());
    assert!(trx.tbl_of::<SoChild>().await?.get(&1).is_none());

    Ok(())
}
