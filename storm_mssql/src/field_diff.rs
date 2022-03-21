use crate::{Error, Result};
use chrono::{FixedOffset, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};
use storm::FieldsOrStr;

/// Create the field from the diff value.
pub trait FromFieldDiff: Sized {
    fn from_field_diff(value: Value) -> Result<Self>;
}

pub trait ApplyFieldDiff: Sized {
    fn apply_field_diff(&mut self, value: Value) -> Result<()>;
}

pub fn apply_field_diff_impl<T: for<'de> Deserialize<'de>>(v: &mut T, value: Value) -> Result<()> {
    *v = from_field_diff_impl(value)?;
    Ok(())
}

pub trait FieldDiff {
    fn field_diff(&self, old: &Self) -> Option<Value>;
}

pub fn field_diff_impl<T: PartialEq + Serialize>(new: &T, old: &T) -> Option<Value> {
    if new == old {
        None
    } else {
        Some(serde_json::json!(old))
    }
}

#[doc(hidden)]
pub fn _replace_field_diff<K: Eq + Hash, T: ApplyFieldDiff, S: BuildHasher>(
    field: &mut T,
    name: K,
    map: &HashMap<FieldsOrStr<K>, Value, S>,
) -> Result<()> {
    if let Some(old) = map.get(&FieldsOrStr::Fields(name)) {
        field.apply_field_diff(old.clone())?;
    }

    Ok(())
}

impl<T: FromFieldDiff> FromFieldDiff for Option<T> {
    fn from_field_diff(value: Value) -> Result<Self> {
        Ok(if value.is_null() {
            None
        } else {
            Some(T::from_field_diff(value)?)
        })
    }
}

impl<T: ApplyFieldDiff + FromFieldDiff> ApplyFieldDiff for Option<T> {
    fn apply_field_diff(&mut self, value: Value) -> Result<()> {
        if value.is_null() {
            *self = None;
            return Ok(());
        }

        match self {
            Some(v) => v.apply_field_diff(value),
            None => {
                *self = Some(T::from_field_diff(value)?);
                Ok(())
            }
        }
    }
}

impl<T: PartialEq + Serialize> FieldDiff for Option<T> {
    fn field_diff(&self, old: &Self) -> Option<Value> {
        field_diff_impl(self, old)
    }
}

pub fn from_field_diff_impl<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(Error::std)
}

macro_rules! diff {
    ($n:ty) => {
        impl ApplyFieldDiff for $n {
            fn apply_field_diff(&mut self, value: Value) -> Result<()> {
                apply_field_diff_impl(self, value)
            }
        }

        impl FieldDiff for $n {
            fn field_diff(&self, old: &Self) -> Option<Value> {
                field_diff_impl(self, old)
            }
        }

        impl FromFieldDiff for $n {
            fn from_field_diff(value: Value) -> Result<Self> {
                from_field_diff_impl(value)
            }
        }
    };
}

diff!(Box<str>);
diff!(String);
diff!(bool);
diff!(chrono::DateTime<FixedOffset>);
diff!(chrono::DateTime<Local>);
diff!(chrono::DateTime<Utc>);
diff!(chrono::NaiveDate);
diff!(chrono::NaiveDateTime);
diff!(chrono::NaiveTime);
diff!(f32);
diff!(f64);
diff!(i16);
diff!(i32);
diff!(i64);
diff!(i8);
diff!(u16);
diff!(u32);
diff!(u64);
diff!(u8);
diff!(uuid::Uuid);

#[cfg(feature = "dec19x5")]
diff!(dec19x5crate::Decimal);
