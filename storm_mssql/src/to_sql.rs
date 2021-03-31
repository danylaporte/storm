use std::borrow::Cow;
use tiberius::ColumnData;

pub trait ToSql: Send + Sync {
    fn to_sql(&self) -> ColumnData;
}

pub trait ToSqlNull {
    fn to_sql_null() -> ColumnData<'static>;
}

impl<T: ToSql> ToSql for &T {
    fn to_sql(&self) -> ColumnData {
        (**self).to_sql()
    }
}

impl<T> ToSql for Option<T>
where
    T: ToSql + ToSqlNull,
{
    fn to_sql(&self) -> ColumnData {
        match self.as_ref() {
            Some(v) => v.to_sql(),
            None => T::to_sql_null(),
        }
    }
}

macro_rules! to_sql {
    (borrowed $t:ty => $n:ident) => {
        impl ToSql for $t {
            #[inline]
            fn to_sql(&self) -> ColumnData {
                ColumnData::$n(Some(Cow::Borrowed(&self)))
            }
        }

        impl ToSqlNull for $t {
            #[inline]
            fn to_sql_null() -> ColumnData<'static> {
                ColumnData::$n(None)
            }
        }
    };
    (copied $t:ty => $n:ident) => {
        impl ToSql for $t {
            #[inline]
            fn to_sql(&self) -> ColumnData {
                ColumnData::$n(Some(*self))
            }
        }

        impl ToSqlNull for $t {
            #[inline]
            fn to_sql_null() -> ColumnData<'static> {
                ColumnData::$n(None)
            }
        }
    };
    (transform $t:ty) => {
        impl ToSql for $t {
            #[inline]
            fn to_sql(&self) -> ColumnData {
                tiberius::ToSql::to_sql(self)
            }
        }

        impl ToSqlNull for $t {
            #[inline]
            fn to_sql_null() -> ColumnData<'static> {
                tiberius::ToSql::to_sql(&Option::<$t>::None)
            }
        }
    };
}

to_sql!(borrowed [u8] => Binary);
to_sql!(borrowed Vec<u8> => Binary);
to_sql!(borrowed str => String);
to_sql!(borrowed &str => String);
to_sql!(borrowed String => String);

to_sql!(copied bool => Bit);
to_sql!(copied f32 => F32);
to_sql!(copied f64 => F64);
to_sql!(copied i16 => I16);
to_sql!(copied i32 => I32);
to_sql!(copied i64 => I64);
to_sql!(copied u8 => U8);

to_sql!(transform chrono::DateTime<chrono::FixedOffset>);
to_sql!(transform chrono::DateTime<chrono::Utc>);
to_sql!(transform chrono::NaiveDate);
to_sql!(transform chrono::NaiveDateTime);
to_sql!(transform chrono::NaiveTime);
to_sql!(transform uuid::Uuid);

#[cfg(feature = "dec19x5")]
impl ToSql for dec19x5::Decimal {
    #[inline]
    fn to_sql(&self) -> ColumnData {
        tiberius::ToSql::to_sql(self)
    }
}

#[cfg(feature = "dec19x5")]
impl ToSqlNull for dec19x5::Decimal {
    #[inline]
    fn to_sql_null() -> ColumnData {
        tiberius::ToSql::to_sql(&Option::<Self>::None)
    }
}
