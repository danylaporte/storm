#![allow(clippy::unwrap_used)]

use std::borrow::Cow;
use storm::{Error, MssqlDelete, MssqlLoad, MssqlSave, Result, prelude::*, provider::Delete};
use storm_mssql::{
    Execute, ExecuteArgs, FromSql, MssqlFactory, MssqlProvider, QueryRows, ToSql, ToSqlNull,
};
use tiberius::Config;
use uuid::Uuid;

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(provider().into(), "ctx")
}
fn provider() -> ProviderContainer {
    let mut config = Config::default();
    config.database("master");
    #[cfg(target_os = "windows")]
    config.authentication(tiberius::AuthMethod::Integrated);
    config.trust_cert();

    let mut provider = ProviderContainer::new();
    provider.register("", MssqlFactory(config));

    provider
}

#[async_cell_lock::tokio_test]
async fn translated_flow() -> storm::Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.read().await?;

    let provider = ctx.provider().provide::<MssqlProvider>("").await?;
    let no_transaction = ExecuteArgs {
        use_transaction: false,
    };

    provider
        .execute_with_args(
            "CREATE TABLE ##Labels (Id Int PRIMARY KEY NOT NULL);",
            &[],
            no_transaction,
        )
        .await?;

    provider.execute_with_args(
            "CREATE TABLE ##LabelsTranslatedValues (Id2 Int NOT NULL, Culture Int NOT NULL, Name NVARCHAR(50) NOT NULL);",
            &[],
            no_transaction,
        )
        .await?;

    let ctx = ctx.queue().await?;

    // translated values are checked per culture, in UTF-16 code units like nvarchar.
    {
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx
            .tbl_of::<Label>()
            .await?
            .insert(
                LabelId(3),
                Label {
                    name: Translated {
                        en: "english".to_owned(),
                        fr: "😀".repeat(26),
                    },
                },
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::FieldTooLong {
                    len: 52,
                    max: 50,
                    ..
                }
            ),
            "{err}"
        );
    }

    let mut trx = ctx.transaction(Uuid::nil());

    let mut labels = trx.tbl_of::<Label>().await?;
    let id = LabelId(2);

    labels
        .insert(
            id,
            Label {
                name: Translated {
                    en: "english".to_owned(),
                    fr: "😀".repeat(25),
                },
            },
        )
        .await?;

    let log = trx.commit().await?;

    let mut ctx = ctx.write().await?;

    ctx.apply_log(log);

    let ctx = ctx.read().await?;
    let v = ctx.tbl_of::<Label>().await?.get(&id);

    assert_eq!(v.map(|v| v.name.fr.as_str()), Some(&*"😀".repeat(25)));
    println!("{v:?}");

    // The delete removes the translated values and the entity.
    let trx = ctx.provider().transaction();
    Delete::<Label>::delete(&trx, &id).await?;
    trx.commit().await?;
    drop(trx);

    let provider = ctx.provider().provide::<MssqlProvider>("").await?;

    for table in ["##Labels", "##LabelsTranslatedValues"] {
        let counts: Vec<i32> = provider
            .query_rows(
                format!("SELECT COUNT(*) FROM {table}"),
                &[],
                |row| i32::from_sql(row.get(0)),
                false,
            )
            .await?;

        assert_eq!(counts, [0], "{table}");
    }

    Ok(())
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "##Labels",
    keys = "Id",
    translate_table = "##LabelsTranslatedValues",
    translate_keys = "Id2",
    no_test = true
)]
struct Label {
    #[storm(max_length = 50)]
    name: Translated,
}

impl Entity for Label {
    type Key = LabelId;
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LabelId(i32);

impl<'a> FromSql<'a> for LabelId {
    type Column = i32;

    fn from_sql(col: Option<Self::Column>) -> Result<Self> {
        match col {
            Some(col) => Ok(LabelId(col)),
            None => Err(Error::ColumnNull),
        }
    }
}

impl From<LabelId> for u32 {
    fn from(id: LabelId) -> Self {
        id.0 as _
    }
}

impl ToSql for LabelId {
    fn to_sql(&self) -> tiberius::ColumnData<'_> {
        self.0.to_sql()
    }
}

impl ToSqlNull for LabelId {
    fn to_sql_null() -> tiberius::ColumnData<'static> {
        tiberius::ColumnData::I32(None)
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
struct Translated {
    en: String,
    fr: String,
}

impl Translated {
    fn get(&self, culture: Culture) -> &str {
        match culture {
            Culture::En => &self.en,
            Culture::Fr => &self.fr,
        }
    }

    fn set<'a>(&mut self, culture: Culture, value: impl Into<Cow<'a, str>>) {
        *(match culture {
            Culture::En => &mut self.en,
            Culture::Fr => &mut self.fr,
        }) = value.into().to_string();
    }
}

impl storm::Gc for Translated {}

#[derive(Clone, Copy, Debug)]
pub enum Culture {
    Fr = 0,
    En = 1,
}

impl Culture {
    pub const DB_CULTURES: [Culture; 2] = [Self::En, Self::Fr];
}

impl<'a> FromSql<'a> for Culture {
    type Column = i32;

    fn from_sql(col: Option<Self::Column>) -> Result<Self> {
        match col {
            Some(0) => Ok(Culture::Fr),
            Some(1) => Ok(Culture::En),
            Some(v) => Err(Error::ConvertFailed(format!("Culture `{v}` invalid."))),
            None => Err(Error::ColumnNull),
        }
    }
}

impl ToSql for Culture {
    fn to_sql(&self) -> tiberius::ColumnData<'_> {
        tiberius::ColumnData::I32(Some(*self as i32))
    }
}

impl ToSqlNull for Culture {
    fn to_sql_null() -> tiberius::ColumnData<'static> {
        tiberius::ColumnData::I32(None)
    }
}
