use crate::{column_equals, column_to_value, Error, FromSql, Result, ToSql};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub trait FieldDiff {
    fn field_diff(&self, old: &Self) -> Option<Value>;
}

impl<T> FieldDiff for T
where
    T: ToSql,
{
    fn field_diff(&self, old: &Self) -> Option<Value> {
        let new = self.to_sql();
        let old = old.to_sql();

        if column_equals(&new, &old) {
            None
        } else {
            Some(column_to_value(&old))
        }
    }
}

pub trait FieldDiffFrom: Sized {
    fn field_diff_from(value: Value) -> Result<Self>;
}

impl<T> FieldDiffFrom for T
where
    for<'a> T: FromSql<'a>,
    for<'a> <T as FromSql<'a>>::Column: DeserializeOwned,
{
    fn field_diff_from(value: Value) -> Result<T> {
        T::from_sql(
            serde_json::from_value::<Option<T::Column>>(value).map_err(|e| Error::Std(e.into()))?,
        )
    }
}
