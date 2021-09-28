use std::{borrow::Cow, ops::Index};
use storm::{prelude::*, Error, MssqlLoad, MssqlSave, Result};
use storm_mssql::{Execute, ExecuteArgs, FromSql, MssqlFactory, MssqlProvider, ToSql, ToSqlNull};
use tiberius::Config;

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(provider().into())
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

#[tokio::test]

async fn translated_flow() -> storm::Result<()> {
    async_cell_lock::with_deadlock_check(async move {
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
        let mut trx = ctx.transaction();

        let mut labels = trx.tbl_of::<Label>().await?;
        let id = LabelId(2);

        labels
            .insert(
                id,
                Label {
                    name: Translated {
                        en: "english".to_owned(),
                        fr: "french".to_owned(),
                    },
                },
                &()
            )
            .await?;

        let log = trx.commit().await?;

        let mut ctx = ctx.write().await?;

        ctx.apply_log(log);

        let ctx = ctx.read().await?;
        let v = ctx.tbl_of::<Label>().await?.get(&id);

        println!("{:?}", v);
        Ok(())
    })
    .await
}

#[derive(Clone, Ctx, Debug, MssqlLoad, MssqlSave)]
#[storm(
    table = "##Labels",
    keys = "Id",
    translate_table = "##LabelsTranslatedValues",
    translate_keys = "Id2",
    no_test = true
)]
struct Label {
    name: Translated,
}

impl Entity for Label {
    type Key = LabelId;
    type TrackCtx = ();
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

impl From<usize> for LabelId {
    fn from(v: usize) -> Self {
        Self(v as _)
    }
}

impl From<LabelId> for usize {
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

#[derive(Clone, Default, Debug)]
struct Translated {
    en: String,
    fr: String,
}

impl Translated {
    fn set<'a>(&mut self, culture: Culture, value: impl Into<Cow<'a, str>>) {
        *(match culture {
            Culture::En => &mut self.en,
            Culture::Fr => &mut self.fr,
        }) = value.into().to_string();
    }
}

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
            Some(v) => Err(Error::ConvertFailed(format!("Culture `{}` invalid.", v))),
            None => Err(Error::ColumnNull),
        }
    }
}

impl Index<Culture> for Translated {
    type Output = str;

    fn index(&self, culture: Culture) -> &str {
        match culture {
            Culture::En => &self.en,
            Culture::Fr => &self.fr,
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
