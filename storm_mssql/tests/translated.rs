use std::borrow::Cow;
use storm::{
    prelude::*, AsyncOnceCell, Connected, Ctx, Entity, Error, MssqlLoad, MssqlSave, QueueRwLock,
    Result,
};
use storm_mssql::{ClientFactory, Execute, FromSql, MssqlProvider};
use tiberius::{AuthMethod, Config, ToSql};
use vec_map::VecMap;

fn create_ctx() -> QueueRwLock<Connected<Ctx, MssqlProvider<Config>>> {
    QueueRwLock::new(Connected {
        ctx: Ctx::default(),
        provider: provider(),
    })
}
fn provider() -> MssqlProvider<Config> {
    let mut config = Config::default();
    config.database("master");
    config.authentication(AuthMethod::Integrated);
    config.trust_cert();
    config.create_provider()
}

#[tokio::test]

async fn translated_flow() -> storm::Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.read().await;

    {
        let t = ctx.provider.transaction().await?;
        t.execute("CREATE TABLE ##Labels (Id Int PRIMARY KEY NOT NULL);", &[])
            .await?;

        t.execute(
            "CREATE TABLE ##LabelsTranslatedValues (Id2 Int NOT NULL, Culture Int NOT NULL, Name NVARCHAR(50) NOT NULL);",
            &[],
        )
        .await?;

        t.commit().await?;
    }

    let ctx = ctx.queue().await;

    let mut trx = ctx.transaction().await?;

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

#[derive(Clone, Copy, Debug)]
pub enum Culture {
    Fr = 0,
    En = 1,
}

impl Culture {
    pub fn iter() -> impl Iterator<Item = Culture> {
        [Culture::Fr, Culture::En].iter().copied()
    }
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

impl ToSql for Culture {
    fn to_sql(&self) -> tiberius::ColumnData<'_> {
        tiberius::ColumnData::I32(Some(*self as i32))
    }
}
