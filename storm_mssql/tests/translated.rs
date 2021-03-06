use std::{borrow::Cow, collections::HashMap};
use storm::{AsyncOnceCell, Connected, Ctx, Entity, Error, MssqlLoad, QueueRwLock, Result};
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

    let _labels = ctx.labels().await?;

    Ok(())
}

#[derive(Ctx, Default)]
struct Ctx {
    labels: AsyncOnceCell<VecMap<LabelId, Label>>,
}

#[derive(Clone, Debug, MssqlLoad)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
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
struct Translated(HashMap<Culture, String>);

impl Translated {
    fn set<'a>(&mut self, culture: Culture, value: impl Into<Cow<'a, str>>) {
        self.0.insert(culture, value.into().to_string());
    }
}

type Culture = i32;
