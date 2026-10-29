#![allow(clippy::unwrap_used)]

use storm::{MssqlDelete, MssqlLoad, MssqlSave, Result, prelude::*};
use storm_mssql::{
    Execute, ExecuteArgs, MssqlProvider, MssqlSchema, check_schema,
    create_provider_container_from_env,
};

// Real tables in tempdb: foreign keys are not enforced on #temp tables.
const SETUP: &str = r"
DROP TABLE IF EXISTS dbo.StormSchemaChild;
DROP TABLE IF EXISTS dbo.StormSchemaOther;
DROP TABLE IF EXISTS dbo.StormSchemaParent;

CREATE TABLE dbo.StormSchemaParent (Id TINYINT NOT NULL PRIMARY KEY, Name NVARCHAR(50) NOT NULL);
CREATE TABLE dbo.StormSchemaOther (Id TINYINT NOT NULL PRIMARY KEY);

CREATE TABLE dbo.StormSchemaChild (
    Id TINYINT NOT NULL PRIMARY KEY,
    ParentId TINYINT NULL CONSTRAINT FK_StormSchemaChild_Parent REFERENCES dbo.StormSchemaOther (Id) ON DELETE SET NULL,
    OwnerId TINYINT NOT NULL CONSTRAINT FK_StormSchemaChild_Owner REFERENCES dbo.StormSchemaParent (Id) ON DELETE CASCADE,
    Email NVARCHAR(100) COLLATE Latin1_General_CI_AS NOT NULL,
    Code VARCHAR(10) COLLATE Latin1_General_CI_AI NOT NULL,
    Login NVARCHAR(30) COLLATE Latin1_General_CS_AS NULL,
    Active BIT NOT NULL,
    Data VARBINARY(16) NULL,
    Notes NVARCHAR(MAX) NULL,
    GroupId INT NOT NULL,
    Number INT NOT NULL,
    Extra INT NOT NULL
);

CREATE UNIQUE INDEX UX_StormSchemaChild_Email ON dbo.StormSchemaChild (Email);
CREATE UNIQUE INDEX UX_StormSchemaChild_Code ON dbo.StormSchemaChild (Code);
CREATE UNIQUE INDEX UX_StormSchemaChild_Login ON dbo.StormSchemaChild (Login) WHERE Active = 1;
CREATE UNIQUE INDEX UX_StormSchemaChild_Group ON dbo.StormSchemaChild (Number, GroupId);
CREATE UNIQUE INDEX UX_StormSchemaChild_IdEmail ON dbo.StormSchemaChild (Id, Email);
";

const CLEANUP: &str = r"
DROP TABLE IF EXISTS dbo.StormSchemaChild;
DROP TABLE IF EXISTS dbo.StormSchemaOther;
DROP TABLE IF EXISTS dbo.StormSchemaParent;
";

async fn exec_in_tempdb(provider: &MssqlProvider, script: &str) -> Result<()> {
    provider
        .execute_with_args(
            "EXEC tempdb.sys.sp_executesql @P1",
            &[&script],
            ExecuteArgs {
                use_transaction: false,
            },
        )
        .await?;

    Ok(())
}

#[tokio::test]
async fn schema_matches_database() -> Result<()> {
    let container = create_provider_container_from_env("DB", "")?;
    let provider = container.provide::<MssqlProvider>("").await?;

    exec_in_tempdb(provider, SETUP).await?;

    let parent = check_schema(provider, &SchemaParent::SCHEMA).await;
    let child = check_schema(provider, &SchemaChild::SCHEMA).await;
    let bad = check_schema(provider, &BadChild::SCHEMA).await;
    let missing = check_schema(provider, &Missing::SCHEMA).await;
    let partial = check_schema(provider, &PartialChild::SCHEMA).await;
    let bad_partial = check_schema(provider, &BadPartialChild::SCHEMA).await;

    exec_in_tempdb(provider, CLEANUP).await?;

    assert_eq!(parent?, Vec::<String>::new());
    assert_eq!(child?, Vec::<String>::new());
    assert_eq!(partial?, Vec::<String>::new());
    assert_eq!(
        bad_partial?,
        [
            "field `code`: column `Code` is varchar(10), add `#[storm(max_length = 10)]`.",
            "field `notes`: column `Notes` is nullable, use an `Option`.",
            "field `number`: column `Number` is not nullable, remove the `Option`.",
        ]
    );
    assert_eq!(
        missing?,
        ["table `tempdb.dbo.StormSchemaMissing` not found."]
    );

    let bad = bad?;
    let expected = [
        "field `email`: max_length = 50 but column `Email` is nvarchar(100).",
        "field `code`: column `Code` is varchar(10), add `#[storm(max_length = 10)]`.",
        "field `data`: column `Data` is varbinary(16), add `#[storm(max_length = 16)]`.",
        "field `notes`: max_length = 4000 but column `Notes` is nvarchar(max), remove `max_length`.",
        "field `missing`: column `Missing` not found in `tempdb.dbo.StormSchemaChild`.",
        "unique index `UX_StormSchemaChild_Code` of `tempdb.dbo.StormSchemaChild` is not declared on the model: add `#[storm(unique(collation = \"ai_ci\"))]` on field `code`.",
        "unique `email`: collation = \"exact\" but column `Email` is `Latin1_General_CI_AS` (collation = \"ci\").",
        "unique index `UX_StormSchemaChild_Group` of `tempdb.dbo.StormSchemaChild` is not declared on the model: add `#[storm(unique(fields = \"number, group_id\"))]` on the type.",
        "unique `login`: index `UX_StormSchemaChild_Login` is filtered by `([Active]=(1))` but the model has no `filter`.",
        "field `owner_id`: references `tempdb.dbo.StormSchemaOther` but foreign key `FK_StormSchemaChild_Owner` references `dbo.StormSchemaParent`.",
        "field `owner_id`: on_delete = \"restrict\" but foreign key `FK_StormSchemaChild_Owner` is on_delete = \"cascade\".",
        "foreign key `FK_StormSchemaChild_Parent` (ParentId -> dbo.StormSchemaOther) of `tempdb.dbo.StormSchemaChild` is not declared on the model: add `#[storm(foreign_key(entity = ..., on_delete = \"set_null\"))]` on field `parent_id`.",
    ];

    assert_eq!(bad, expected, "{bad:#?}");

    Ok(())
}

#[tokio::test]
async fn generated_test_entity() {
    let container = create_provider_container_from_env("DB", "").unwrap();
    let provider = container.provide::<MssqlProvider>("").await.unwrap();

    exec_in_tempdb(provider, &SETUP.replace("StormSchema", "StormSchemaGen"))
        .await
        .unwrap();

    // panics on a mismatch.
    storm_mssql::test_entity::<GenParent>("").await;

    exec_in_tempdb(provider, &CLEANUP.replace("StormSchema", "StormSchemaGen"))
        .await
        .unwrap();
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "tempdb.dbo.StormSchemaParent",
    keys = "Id",
    rename_all = "PascalCase",
    no_test = true
)]
struct SchemaParent {
    #[storm(max_length = 50)]
    name: String,
}

impl Entity for SchemaParent {
    type Key = u8;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "tempdb.dbo.StormSchemaGenParent",
    keys = "Id",
    rename_all = "PascalCase",
    no_test = true
)]
struct GenParent {
    #[storm(max_length = 50)]
    name: String,
}

impl Entity for GenParent {
    type Key = u8;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(table = "tempdb.dbo.StormSchemaOther", keys = "Id", no_test = true)]
struct SchemaOther {}

impl Entity for SchemaOther {
    type Key = u8;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "tempdb.dbo.StormSchemaChild",
    keys = "Id",
    rename_all = "PascalCase",
    no_test = true,
    unique(fields = "group_id, number"),
    unique(name = "login", fields = "login", collation = "exact", filter = |c: &SchemaChild| c.active),
)]
struct SchemaChild {
    #[storm(foreign_key(entity = SchemaOther, on_delete = "set_null"))]
    parent_id: Option<u8>,

    #[storm(foreign_key(entity = SchemaParent, on_delete = "cascade"))]
    owner_id: u8,

    #[storm(unique, max_length = 100)]
    email: String,

    #[storm(unique(collation = "ai_ci"), max_length = 10)]
    code: String,

    #[storm(max_length = 30)]
    login: Option<String>,

    active: bool,

    #[storm(max_length = 16)]
    data: Option<Vec<u8>>,

    notes: Option<String>,
    group_id: i32,
    number: i32,

    // model only unique indexes are allowed.
    #[storm(unique)]
    extra: i32,
}

impl Entity for SchemaChild {
    type Key = u8;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "tempdb.dbo.StormSchemaChild",
    keys = "Id",
    rename_all = "PascalCase",
    no_test = true,
    unique(name = "login", fields = "login", collation = "exact")
)]
struct BadChild {
    // missing foreign key.
    parent_id: Option<u8>,

    #[storm(foreign_key(entity = SchemaOther))]
    owner_id: u8,

    #[storm(unique(collation = "exact"), max_length = 50)]
    email: String,

    code: String,

    #[storm(max_length = 30)]
    login: Option<String>,

    active: bool,
    data: Option<Vec<u8>>,

    #[storm(max_length = 4000)]
    notes: Option<String>,

    group_id: i32,
    number: i32,
    missing: i32,
}

impl Entity for BadChild {
    type Key = u8;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(table = "tempdb.dbo.StormSchemaMissing", keys = "Id", no_test = true)]
struct Missing {}

impl Entity for Missing {
    type Key = u8;
}

// Unique indexes and foreign keys of the table are not checked.
#[derive(MssqlLoad)]
#[storm(
    table = "tempdb.dbo.StormSchemaChild",
    keys = "Id",
    rename_all = "PascalCase",
    no_test = true,
    partial = true
)]
#[allow(dead_code)]
struct PartialChild {
    #[storm(max_length = 100)]
    email: String,

    #[storm(max_length = 30)]
    login: Option<String>,

    #[storm(max_length = 16)]
    data: Option<Vec<u8>>,

    // nullable column mapped by a non-`Option` type.
    #[storm(nullable = true)]
    notes: String,
}

impl Entity for PartialChild {
    type Key = u8;
}

#[derive(MssqlLoad)]
#[storm(
    table = "tempdb.dbo.StormSchemaChild",
    keys = "Id",
    rename_all = "PascalCase",
    no_test = true,
    partial = true
)]
#[allow(dead_code)]
struct BadPartialChild {
    code: String,
    notes: String,
    number: Option<i32>,
}

impl Entity for BadPartialChild {
    type Key = u8;
}
