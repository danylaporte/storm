use chrono::{NaiveDate, NaiveDateTime};
use storm::Result;
use tiberius::Uuid;

pub trait FromSql<'a>: Sized {
    type Column: tiberius::FromSql<'a>;

    fn from_sql(col: Option<Self::Column>) -> Result<Self>;
}

impl<'a, T> FromSql<'a> for Option<T>
where
    T: FromSql<'a>,
{
    type Column = T::Column;

    fn from_sql(col: Option<Self::Column>) -> Result<Self> {
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

            fn from_sql(col: Option<Self::Column>) -> Result<Self> {
                match col {
                    Some(v) => Ok(v),
                    None => Err(storm::Error::ColumnNull),
                }
            }
        }
    };
}

from_sql!(&'a str, &'a str);
from_sql!(NaiveDate, NaiveDate);
from_sql!(NaiveDateTime, NaiveDateTime);
from_sql!(Uuid, Uuid);
from_sql!(bool, bool);
from_sql!(i16, i16);
from_sql!(i32, i32);
from_sql!(i64, i64);
from_sql!(u8, u8);
from_sql!(&'a [u8], &'a [u8]);

impl<'a> FromSql<'a> for String {
    type Column = &'a str;

    fn from_sql(col: Option<Self::Column>) -> Result<Self> {
        match col {
            Some(v) => Ok(v.to_owned()),
            None => Err(storm::Error::ColumnNull),
        }
    }
}

impl<'a> FromSql<'a> for Vec<u8> {
    type Column = &'a [u8];

    fn from_sql(col: Option<Self::Column>) -> Result<Self> {
        match col {
            Some(v) => Ok(v.to_owned()),
            None => Err(storm::Error::ColumnNull),
        }
    }
}
