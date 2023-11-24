use std::borrow::Cow;
use storm::{prelude::*, MssqlLoad, MssqlSave};
use storm_mssql::{
    Execute, ExecuteArgs, FromSql, FromSqlError, MssqlFactory, MssqlProvider, ToSql, ToSqlNull,
};
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

async fn identity_translate_key() -> storm::Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let ctx = create_ctx();
        let ctx = ctx.read().await?;

        let provider = ctx.provider().provide::<MssqlProvider>("").await?;
        let no_transaction = ExecuteArgs {
            use_transaction: false,
        };

        provider
            .execute_with_args(
                "CREATE TABLE ##Labels (Id Int NOT NULL IDENTITY);".to_string(),
                &[],
                no_transaction,
            )
            .await?;

        provider.execute_with_args(
            "CREATE TABLE ##LabelsTranslatedValues (Id2 Int NOT NULL, Culture Int NOT NULL, Name NVARCHAR(50) NOT NULL);".to_string(),
            &[],
            no_transaction,
        )
        .await?;

        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction();

        let mut labels = trx.tbl_of::<Label>().await?;
        let id = 2;

        labels
            .insert_mut(
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
        let v = ctx.tbl_of::<Label>().await?.get(&id).unwrap();

        assert_eq!(v.name.en, "english");

        println!("{v:?}");
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
    collection = "hash_table",
    identity = "Id",
    no_test = true
)]
struct Label {
    name: Translated,
}

impl Entity for Label {
    type Key = i32;
    type TrackCtx = ();
}

#[derive(Clone, Default, Debug)]
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

    fn from_sql(col: Option<Self::Column>) -> std::result::Result<Self, FromSqlError> {
        match col {
            Some(0) => Ok(Culture::Fr),
            Some(1) => Ok(Culture::En),
            Some(v) => Err(FromSqlError::Unexpected {
                ty: "Culture",
                value: v.to_string(),
            }),
            None => Err(FromSqlError::ColumnNull),
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
