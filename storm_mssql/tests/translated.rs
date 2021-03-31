use std::{borrow::Cow, ops::Index};
use storm::{
    prelude::*, provider::ProviderContainer, AsyncOnceCell, Connected, Ctx, Entity, Error,
    MssqlLoad, MssqlSave, QueueRwLock, Result,
};
use storm_mssql::{Execute, FromSql, MssqlFactory, MssqlProvider, ToSql, ToSqlNull};
use tiberius::{AuthMethod, Config};
use vec_map::VecMap;

fn create_ctx() -> QueueRwLock<Connected<Ctx>> {
    QueueRwLock::new(Connected {
        ctx: Ctx::default(),
        provider: provider(),
    })
}
fn provider() -> ProviderContainer {
    let mut config = Config::default();
    config.database("master");
    config.authentication(AuthMethod::Integrated);
    config.trust_cert();

    let mut provider = ProviderContainer::new();
    provider.register("", MssqlFactory(config));

    provider
}

#[tokio::test]

async fn translated_flow() -> storm::Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.read().await;

    let provider = ctx.provider.provide::<MssqlProvider>("").await?;

    provider
        .execute("CREATE TABLE ##Labels (Id Int PRIMARY KEY NOT NULL);", &[])
        .await?;

    provider.execute(
        "CREATE TABLE ##LabelsTranslatedValues (Id2 Int NOT NULL, Culture Int NOT NULL, Name NVARCHAR(50) NOT NULL);",
        &[],
    )
    .await?;

    let ctx = ctx.queue().await;

    let mut trx = ctx.transaction();

    let mut labels = trx.labels_mut().await?;
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
        )
        .await?;

    let log = trx.commit().await?;

    let mut ctx = ctx.write().await;

    ctx.apply_log(log);

    let ctx = ctx.read().await;

    let v = ctx.labels().await?.get(&id);
    println!("{:?}", v);

    //let _labels = ctx.labels().await?;

    Ok(())
}

#[derive(Ctx, Default)]
struct Ctx {
    labels: AsyncOnceCell<VecMap<LabelId, Label>>,
}

#[derive(Clone, Debug, MssqlLoad, MssqlSave)]
#[storm(
    table = "##Labels",
    keys = "Id",
    translate_table = "##LabelsTranslatedValues",
    translate_keys = "Id2"
)]
struct Label {
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
