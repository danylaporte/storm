use crate::FromSqlError;
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use std::{borrow::Cow, sync::Arc};
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

/// Internal used for macros
#[doc(hidden)]
pub fn _macro_load_field<'a, T: FromSql<'a>>(
    row: &'a tiberius::Row,
    index: usize,
    column: &'static str,
    table: &'static str,
) -> storm::Result<T> {
    row.try_get(index)
        .map_err(crate::Error::unknown)
        .and_then(|col| {
            FromSql::from_sql(col).map_err(|err| crate::Error::from_sql(err, column, table))
        })
        .map_err(Into::into)
}
