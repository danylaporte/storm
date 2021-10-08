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

/// Extract a Json value from the field diff.
pub trait ValueFieldDiff {
    fn value_field_diff(&self) -> Result<Value>;
}

pub trait ApplyFieldDiff: Sized {
    fn apply_field_diff(&mut self, value: Value) -> Result<Value>;
}

pub fn apply_field_diff_impl<T: FromFieldDiff + ValueFieldDiff>(
    v: &mut T,
    value: Value,
) -> Result<Value> {
    let new = v.value_field_diff()?;
    *v = T::from_field_diff(value)?;
    Ok(new)
}

pub trait FieldDiff {
    fn field_diff(&self, old: &Self) -> Option<Value>;
}

pub fn field_diff_impl<T: PartialEq + Serialize>(new: &T, old: &T) -> Option<Value> {
    if new == old {
        None
    } else {
        Some(serde_json::to_value(old).expect("diff"))
    }
}

#[doc(hidden)]
pub fn _replace_field_diff<K: Eq + Hash, T: ApplyFieldDiff, S: BuildHasher>(
    field: &mut T,
    name: K,
    map: &mut HashMap<FieldsOrStr<K>, Value, S>,
) -> Result<()> {
    if let Some((key, old)) = map.remove_entry(&FieldsOrStr::Fields(name)) {
        let new = field.apply_field_diff(old)?;
        map.insert(key, new);
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

impl<T: ValueFieldDiff> ValueFieldDiff for Option<T> {
    fn value_field_diff(&self) -> Result<Value> {
        match self {
            Some(v) => v.value_field_diff(),
            None => Ok(Value::Null),
        }
    }
}

impl<T: FromFieldDiff + ValueFieldDiff> ApplyFieldDiff for Option<T> {
    fn apply_field_diff(&mut self, value: Value) -> Result<Value> {
        apply_field_diff_impl(self, value)
    }
}

impl<T: PartialEq + Serialize> FieldDiff for Option<T> {
    fn field_diff(&self, old: &Self) -> Option<Value> {
        field_diff_impl(self, old)
    }
}

fn from_value<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(Error::std)
}

fn to_value<T: Serialize>(value: &T) -> Result<Value> {
    serde_json::to_value(value).map_err(Error::std)
}

macro_rules! diff {
    ($n:ty) => {
        impl ApplyFieldDiff for $n {
            fn apply_field_diff(&mut self, value: Value) -> Result<Value> {
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
                from_value(value)
            }
        }

        impl ValueFieldDiff for $n {
            fn value_field_diff(&self) -> Result<Value> {
                to_value(self)
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
diff!(dec19x5::Decimal);
