use crate::Error;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde::Deserialize;
use std::{any::type_name, borrow::Cow, fmt::Display, sync::Arc};
use tiberius::Uuid;

pub trait FromSql<'a>: Sized {
    type Column: tiberius::FromSql<'a>;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError>;
}

impl<'a, T> FromSql<'a> for Option<T>
where
    T: FromSql<'a>,
{
    type Column = T::Column;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        Ok(match col {
            Some(v) => Some(T::from_sql(Some(v))?),
            None => None,
        })
    }
}

macro_rules! from_sql {
    ($s:ty, $col:ty) => {
        impl<'a> FromSql<'a> for $s {
            type Column = $col;

            fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
                match col {
                    Some(v) => Ok(v),
                    None => Err(FromSqlError::ColumnNull),
                }
            }
        }
    };
}

from_sql!(&'a str, &'a str);
from_sql!(DateTime<FixedOffset>, DateTime<FixedOffset>);
from_sql!(DateTime<Utc>, DateTime<Utc>);
from_sql!(NaiveDate, NaiveDate);
from_sql!(NaiveDateTime, NaiveDateTime);
from_sql!(NaiveTime, NaiveTime);
from_sql!(Uuid, Uuid);
from_sql!(bool, bool);
from_sql!(f32, f32);
from_sql!(f64, f64);
from_sql!(i16, i16);
from_sql!(i32, i32);
from_sql!(i64, i64);
from_sql!(u8, u8);
from_sql!(&'a [u8], &'a [u8]);

#[cfg(feature = "dec19x5")]
impl<'a> FromSql<'a> for dec19x5crate::Decimal {
    type Column = dec19x5crate::Decimal;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        match col {
            Some(v) => Ok(v),
            None => Err(FromSqlError::ColumnNull),
        }
    }
}

impl<'a> FromSql<'a> for String {
    type Column = &'a str;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        match col {
            Some(v) => Ok(v.to_owned()),
            None => Err(FromSqlError::ColumnNull),
        }
    }
}

impl<'a> FromSql<'a> for Vec<u8> {
    type Column = &'a [u8];

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        match col {
            Some(v) => Ok(v.to_owned()),
            None => Err(FromSqlError::ColumnNull),
        }
    }
}

impl<'a, T: FromSql<'a>> FromSql<'a> for Arc<T> {
    type Column = T::Column;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        T::from_sql(col).map(Arc::new)
    }
}

impl<'a, T: FromSql<'a>> FromSql<'a> for Box<T> {
    type Column = T::Column;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        T::from_sql(col).map(Box::new)
    }
}

impl<'a, T: Clone + FromSql<'a>> FromSql<'a> for Cow<'a, T> {
    type Column = T::Column;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        T::from_sql(col).map(Cow::Owned)
    }
}

impl<'a> FromSql<'a> for Box<[u8]> {
    type Column = &'a [u8];

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        match col {
            Some(col) => Ok(col.to_vec().into_boxed_slice()),
            None => Err(FromSqlError::ColumnNull),
        }
    }
}

impl<'a> FromSql<'a> for Box<str> {
    type Column = &'a str;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        match col {
            Some(col) => Ok(col.to_string().into_boxed_str()),
            None => Err(FromSqlError::ColumnNull),
        }
    }
}

impl<'a, 'b> FromSql<'a> for Cow<'b, [u8]> {
    type Column = &'a [u8];

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        match col {
            Some(col) => Ok(Cow::Owned(col.to_vec())),
            None => Err(FromSqlError::ColumnNull),
        }
    }
}

impl<'a, 'b> FromSql<'a> for Cow<'b, str> {
    type Column = &'a str;

    fn from_sql(col: Option<Self::Column>) -> Result<Self, FromSqlError> {
        match col {
            Some(col) => Ok(Cow::Owned(col.to_string())),
            None => Err(FromSqlError::ColumnNull),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FromSqlError {
    #[error("column is null")]
    ColumnNull,

    #[error("failed convert from {value}, {error}")]
    ConvertFrom {
        #[source]
        error: Box<dyn std::error::Error + Send + Sync>,
        value: String,
    },

    #[error("{0}")]
    Custom(String),

    #[error("failed decrypt {0}")]
    Decrypt(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("failed deserialize json, {error}\n{payload}")]
    Dejson {
        #[source]
        error: serde_json::Error,
        payload: String,
    },

    #[error("failed deserialize {0}")]
    Deserialize(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("invalid value {0}")]
    InvalidValue(String),

    #[error("invalid value {value}, {error}")]
    InvalidValueWithErr {
        #[source]
        error: Box<dyn std::error::Error + Send + Sync>,
        value: String,
    },

    #[error(transparent)]
    Unknown(Box<dyn std::error::Error + Send + Sync>),
}

impl FromSqlError {
    pub fn decrypt<E>(error: E) -> FromSqlError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Decrypt(Box::new(error))
    }

    pub fn deserialize<E>(error: E) -> FromSqlError
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::Deserialize(Box::new(error))
    }

    pub fn invalid_value<V: Display>(value: V) -> Self {
        Self::InvalidValue(value.to_string())
    }

    pub fn invalid_value_with_err<V, E>(value: V, error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
        V: Display,
    {
        Self::InvalidValueWithErr {
            error: Box::new(error),
            value: value.to_string(),
        }
    }

    pub fn unknown<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        Self::Unknown(Box::new(e))
    }
}

pub fn convert_from_copy<S, T>(val: S) -> Result<T, FromSqlError>
where
    S: Copy + Display,
    T: TryFrom<S>,
    T::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    match TryFrom::try_from(val) {
        Ok(v) => Ok(v),
        Err(e) => Err(FromSqlError::ConvertFrom {
            error: e.into(),
            value: val.to_string(),
        }),
    }
}

/// Deserialize a column content in json wrapping the error for maximum tracability.
pub fn dejson<T>(s: &str) -> Result<T, FromSqlError>
where
    T: for<'de> Deserialize<'de>,
{
    match serde_json::from_str(s) {
        Ok(v) => Ok(v),
        Err(error) => Err(FromSqlError::Dejson {
            error,
            payload: s.to_string(),
        }),
    }
}

/// Internal used for macros
#[doc(hidden)]
pub fn _macro_load_field<'a, T: FromSql<'a>>(
    row: &'a tiberius::Row,
    index: usize,
    column: &'static str,
    table: &'static str,
) -> storm::Result<T> {
    row.try_get(index)
        .map_err(Error::unknown)
        .and_then(|col| {
            FromSql::from_sql(col).map_err(|error| Error::FromSql {
                column,
                error,
                table,
                ty: type_name::<T>(),
            })
        })
        .map_err(Into::into)
}
