use storm::{
    Collation, Error, NoopDelete, NoopLoad, NoopSave, Result, Unique, UniqueValue, prelude::*,
};
use uuid::Uuid;

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(Default::default(), "ctx")
}

fn is_violation(r: Result<impl Sized>, index: &str) -> bool {
    matches!(r, Err(Error::UniqueViolation { index: i, .. }) if i == index)
}

#[async_cell_lock::tokio_test]
async fn single_field_case_insensitive_by_default() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(
            1,
            User {
                email: "john@x.com".into(),
            },
        )
        .await?;

        // conflict with an uncommitted row.
        {
            let mut trx = ctx.transaction(Uuid::nil());
            trx.insert(
                1,
                User {
                    email: "john@x.com".into(),
                },
            )
            .await?;
            let err = trx
                .insert(
                    2,
                    User {
                        email: "JOHN@X.COM".into(),
                    },
                )
                .await;
            assert!(is_violation(err, "email"));
        }

        // accent sensitive.
        trx.insert(
            2,
            User {
                email: "jöhn@x.com".into(),
            },
        )
        .await?;

        // updating the same row (even changing case) is allowed.
        trx.insert(
            1,
            User {
                email: "John@X.com".into(),
            },
        )
        .await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    {
        let ctx = ctx.queue().await?;

        {
            // conflict with a committed row.
            let mut trx = ctx.transaction(Uuid::nil());
            let err = trx
                .insert(
                    3,
                    User {
                        email: "john@x.COM".into(),
                    },
                )
                .await
                .unwrap_err();

            assert_eq!(
                err.to_string(),
                "Unique index violation: User `email` (email) value \"john@x.COM\" is already used by `1`."
            );
        }

        // removing or changing the existing row frees the value.
        let mut trx = ctx.transaction(Uuid::nil());
        trx.remove::<User>(1).await?;
        trx.insert(
            3,
            User {
                email: "john@x.com".into(),
            },
        )
        .await?;
        trx.insert(
            3,
            User {
                email: "other@x.com".into(),
            },
        )
        .await?;
        trx.insert(
            4,
            User {
                email: "john@x.com".into(),
            },
        )
        .await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.read().await?;
    let idx = ctx.ref_as::<UserEmailUniqueIndex>().await?;
    assert!(idx.lookup("JOHN@x.com").contains(4));
    assert!(idx.lookup("other@x.com").contains(3));
    assert_eq!(idx.keys().count(), 3);
    assert_eq!(
        idx.lookup(" JOHN@x.com ").iter().collect::<Vec<_>>(),
        vec![4]
    );
    assert!(idx.lookup("nobody@x.com").is_empty());

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn collations() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;

    {
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(
            1,
            Tag {
                name: "Café".into(),
            },
        )
        .await?;
        assert!(is_violation(
            trx.insert(
                2,
                Tag {
                    name: "CAFE".into()
                }
            )
            .await,
            "name"
        ));
    }

    {
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(1, Code { code: "abc".into() }).await?;
        trx.insert(2, Code { code: "ABC".into() }).await?;
        assert!(is_violation(
            trx.insert(3, Code { code: "abc".into() }).await,
            "code"
        ));
    }

    {
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(
            1,
            Word {
                word: "Élan".into(),
            },
        )
        .await?;
        trx.insert(
            2,
            Word {
                word: "élan".into(),
            },
        )
        .await?;
        assert!(is_violation(
            trx.insert(
                3,
                Word {
                    word: "Elan".into()
                }
            )
            .await,
            "word"
        ));

        let idx = trx.index::<WordIdsByWord>().await?;
        assert!(idx.lookup("ELAN").is_empty());
        assert!(idx.lookup("Elan").contains(1));
        assert!(idx.lookup("elan").contains(2));
    }

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn multiple_fields_with_none_as_value() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(
            1,
            Employee {
                company_id: 1,
                code: Some("a".into()),
            },
        )
        .await?;
        trx.insert(
            2,
            Employee {
                company_id: 2,
                code: Some("a".into()),
            },
        )
        .await?;
        trx.insert(
            3,
            Employee {
                company_id: 1,
                code: None,
            },
        )
        .await?;
        trx.insert(
            4,
            Employee {
                company_id: 2,
                code: None,
            },
        )
        .await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.queue().await?;

    {
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx
            .insert(
                5,
                Employee {
                    company_id: 1,
                    code: Some("A".into()),
                },
            )
            .await;
        assert!(is_violation(err, "company_id_code"));
    }

    {
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx
            .insert(
                5,
                Employee {
                    company_id: 1,
                    code: None,
                },
            )
            .await;
        assert!(is_violation(err, "company_id_code"));
    }

    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(
        5,
        Employee {
            company_id: 3,
            code: None,
        },
    )
    .await?;

    assert_eq!(
        storm::unique::find::<EmployeeCompanyIdCodeUnique>(&mut trx, &(1u32, Some(" A "))).await?,
        vec![1]
    );

    let idx = trx.index::<EmployeeCompanyIdCodeUniqueIndex>().await?;
    assert!(idx.contains(&(3, None), 5));
    assert!(idx.lookup(&(2u32, Some("a"))).contains(2));

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn filtered() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;

    {
        let mut trx = ctx.transaction(Uuid::nil());

        // inactive rows are not indexed.
        trx.insert(
            1,
            Account {
                login: "bob".into(),
                active: false,
                archived: true,
            },
        )
        .await?;
        trx.insert(
            2,
            Account {
                login: "bob".into(),
                active: false,
                archived: true,
            },
        )
        .await?;
        trx.insert(
            3,
            Account {
                login: "bob".into(),
                active: true,
                archived: false,
            },
        )
        .await?;

        let err = trx
            .insert(
                1,
                Account {
                    login: "Bob".into(),
                    active: true,
                    archived: false,
                },
            )
            .await;
        assert!(is_violation(err, "login"));
    }

    {
        // type level unique with a named filter function.
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(
            1,
            Account {
                login: "a".into(),
                active: false,
                archived: false,
            },
        )
        .await?;
        trx.insert(
            2,
            Account {
                login: "a".into(),
                active: false,
                archived: true,
            },
        )
        .await?;

        let err = trx
            .insert(
                3,
                Account {
                    login: "A".into(),
                    active: false,
                    archived: false,
                },
            )
            .await;
        assert!(is_violation(err, "visible_login"));
    }

    Ok(())
}

#[test]
fn metadata() {
    assert_eq!(UserEmailUnique::FIELDS, &["email"]);
    assert_eq!(UserEmailUnique::COLLATION, Collation::Ci);
    assert_eq!(TagNameUnique::COLLATION, Collation::AiCi);
    assert_eq!(CodeCodeUnique::COLLATION, Collation::Exact);
    assert_eq!(WordWordUnique::COLLATION, Collation::Ai);
    assert_eq!(EmployeeCompanyIdCodeUnique::FIELDS, &["company_id", "code"]);
    assert_eq!(AccountVisibleLoginUnique::NAME, "visible_login");

    assert!(UserEmailUnique::TRIM);
    assert!(!TagNameUnique::TRIM);
    assert!(EmployeeCompanyIdCodeUnique::TRIM);

    let user = User { email: "x".into() };
    assert_eq!(
        UserEmailUnique::unique_key(&user),
        Some("X ".unique_key(Collation::Ci, true))
    );
    assert_ne!(
        "x".unique_key(Collation::Exact, false),
        "x".unique_key(Collation::Ci, false)
    );
    assert_ne!(
        " x".unique_key(Collation::Ci, false),
        " x".unique_key(Collation::Ci, true)
    );
}

#[async_cell_lock::tokio_test]
async fn trim_on_upsert() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());

    trx.insert(
        1,
        User {
            email: "  john@x.com ".into(),
        },
    )
    .await?;
    assert!(is_violation(
        trx.insert(
            2,
            User {
                email: "john@x.com".into()
            }
        )
        .await,
        "email"
    ));

    let users = trx.tbl_of::<User>().await?;
    assert_eq!(users.get(&1).map(|u| u.email.as_str()), Some("john@x.com"));

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn all_violations_reported_with_fields() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());
    let member = || Member {
        email: "a@x.com".into(),
        login: "a".into(),
    };

    trx.insert(1, member()).await?;

    let Err(Error::Multiple(errors)) = trx.insert(2, member()).await else {
        panic!("expected multiple errors");
    };

    let fields = errors
        .iter()
        .map(|e| match e {
            Error::UniqueViolation { entity_fields, .. } => entity_fields
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>(),
            e => panic!("{e:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(fields, vec![vec!["Email"], vec!["Login"]]);

    // no fields enum.
    let mut trx = ctx.transaction(Uuid::nil());
    trx.insert(2, User { email: "x".into() }).await?;

    let Err(Error::UniqueViolation { entity_fields, .. }) =
        trx.insert(3, User { email: "x".into() }).await
    else {
        panic!("expected a unique violation");
    };

    assert!(entity_fields.is_empty());

    Ok(())
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct Member {
    #[storm(unique)]
    email: String,
    #[storm(unique)]
    login: String,
}

impl Entity for Member {
    type Key = u32;
}

#[derive(Debug)]
enum MemberFields {
    Email,
    Login,
}

impl std::fmt::Display for MemberFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl storm::Fields for MemberFields {}

impl storm::EntityFields for Member {
    type Fields = MemberFields;

    fn field_by_name(name: &str) -> Option<Self::Fields> {
        match name {
            "Email" => Some(MemberFields::Email),
            "Login" => Some(MemberFields::Login),
            _ => None,
        }
    }
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct User {
    #[storm(trim, unique)]
    email: String,
}

impl Entity for User {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct Tag {
    #[storm(unique(collation = "ai_ci"))]
    name: String,
}

impl Entity for Tag {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct Code {
    #[storm(unique(collation = "exact"))]
    code: Box<str>,
}

impl Entity for Code {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct Word {
    #[storm(unique(collation = "ai", index = "WordIdsByWord"))]
    word: String,
}

impl Entity for Word {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(collection = "hash_table")]
#[storm(unique(fields = "company_id, code"))]
struct Employee {
    company_id: u32,
    #[storm(trim)]
    code: Option<String>,
}

impl Entity for Employee {
    type Key = u32;
}

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(unique(name = "visible_login", fields = "login", filter = "is_visible"))]
struct Account {
    #[storm(unique(filter = |a: &Account| a.active))]
    login: String,
    active: bool,
    archived: bool,
}

impl Entity for Account {
    type Key = u32;
}

fn is_visible(a: &Account) -> bool {
    !a.active && !a.archived
}

// unique without index, on a composite key entity

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
#[storm(collection = "hash_table")]
struct NoIndexHash {
    #[storm(unique(no_index))]
    hash: String,
}

impl Entity for NoIndexHash {
    type Key = (u32, u32);
}

#[async_cell_lock::tokio_test]
async fn unique_no_index_on_composite_key() -> Result<()> {
    let ctx = create_ctx();

    {
        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert((1, 1), NoIndexHash { hash: "abc".into() }).await?;
        trx.insert((1, 2), NoIndexHash { hash: "def".into() }).await?;

        // conflict with an uncommitted row, case insensitive.
        {
            let mut trx = ctx.transaction(Uuid::nil());
            trx.insert((1, 1), NoIndexHash { hash: "abc".into() }).await?;
            let err = trx.insert((2, 1), NoIndexHash { hash: "ABC".into() }).await;
            assert!(is_violation(err, "hash"));
        }

        // updating the same row is allowed.
        trx.insert((1, 1), NoIndexHash { hash: "Abc".into() }).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    let ctx = ctx.queue().await?;

    // conflict with a committed row.
    {
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx.insert((2, 1), NoIndexHash { hash: "def".into() }).await;
        assert!(is_violation(err, "hash"));
    }

    let mut trx = ctx.transaction(Uuid::nil());
    assert_eq!(
        storm::unique::find::<NoIndexHashHashUnique>(&mut trx, "ABC").await?,
        vec![(1, 1)]
    );

    // the value is released by the removal in the transaction.
    trx.remove::<NoIndexHash>((1, 2)).await?;
    trx.insert((2, 1), NoIndexHash { hash: "def".into() }).await?;

    Ok(())
}
