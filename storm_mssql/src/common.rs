use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};
use serde_json::Value;
use std::borrow::Cow;
use tiberius::{ColumnData, FromSql};

pub(crate) fn column_equals(a: &ColumnData<'_>, b: &ColumnData<'_>) -> bool {
    match a {
        ColumnData::Binary(a) => match b {
            ColumnData::Binary(b) => a == b,
            _ => false,
        },
        ColumnData::Bit(a) => match b {
            ColumnData::Bit(b) => a == b,
            _ => false,
        },
        ColumnData::Date(a) => match b {
            ColumnData::Date(b) => a == b,
            _ => false,
        },
        ColumnData::DateTime(a) => match b {
            ColumnData::DateTime(b) => a == b,
            _ => false,
        },
        ColumnData::DateTime2(a) => match b {
            ColumnData::DateTime2(b) => a == b,
            _ => false,
        },
        ColumnData::DateTimeOffset(a) => match b {
            ColumnData::DateTimeOffset(b) => a == b,
            _ => false,
        },
        ColumnData::F32(a) => match b {
            ColumnData::F32(b) => a == b,
            _ => false,
        },
        ColumnData::F64(a) => match b {
            ColumnData::F64(b) => a == b,
            _ => false,
        },
        ColumnData::Guid(a) => match b {
            ColumnData::Guid(b) => a == b,
            _ => false,
        },
        ColumnData::I16(a) => match b {
            ColumnData::I16(b) => a == b,
            _ => false,
        },
        ColumnData::I32(a) => match b {
            ColumnData::I32(b) => a == b,
            _ => false,
        },
        ColumnData::I64(a) => match b {
            ColumnData::I64(b) => a == b,
            _ => false,
        },
        ColumnData::Numeric(a) => match b {
            ColumnData::Numeric(b) => a == b,
            _ => false,
        },
        ColumnData::SmallDateTime(a) => match b {
            ColumnData::SmallDateTime(b) => a == b,
            _ => false,
        },
        ColumnData::String(a) => match b {
            ColumnData::String(b) => a == b,
            _ => false,
        },
        ColumnData::Time(a) => match b {
            ColumnData::Time(b) => a == b,
            _ => false,
        },
        ColumnData::U8(a) => match b {
            ColumnData::U8(b) => a == b,
            _ => false,
        },
        ColumnData::Xml(a) => match b {
            ColumnData::Xml(b) => a == b,
            _ => false,
        },
    }
}

pub(crate) fn column_to_owned(v: &ColumnData<'_>) -> ColumnData<'static> {
    match v {
        ColumnData::Binary(Some(v)) => ColumnData::Binary(Some(Cow::Owned(v.to_vec()))),
        ColumnData::Binary(None) => ColumnData::Binary(None),
        ColumnData::Bit(v) => ColumnData::Bit(*v),
        ColumnData::Date(v) => ColumnData::Date(*v),
        ColumnData::DateTime(v) => ColumnData::DateTime(*v),
        ColumnData::DateTime2(v) => ColumnData::DateTime2(*v),
        ColumnData::DateTimeOffset(v) => ColumnData::DateTimeOffset(*v),
        ColumnData::F32(v) => ColumnData::F32(*v),
        ColumnData::F64(v) => ColumnData::F64(*v),
        ColumnData::Guid(v) => ColumnData::Guid(*v),
        ColumnData::I16(v) => ColumnData::I16(*v),
        ColumnData::I32(v) => ColumnData::I32(*v),
        ColumnData::I64(v) => ColumnData::I64(*v),
        ColumnData::Numeric(v) => ColumnData::Numeric(*v),
        ColumnData::SmallDateTime(v) => ColumnData::SmallDateTime(*v),
        ColumnData::String(Some(s)) => ColumnData::String(Some(Cow::Owned(s.to_string()))),
        ColumnData::String(None) => ColumnData::String(None),
        ColumnData::Time(v) => ColumnData::Time(*v),
        ColumnData::U8(v) => ColumnData::U8(*v),
        ColumnData::Xml(_) => panic!("xml is not supported"),
    }
}

pub(crate) fn column_to_value(data: &ColumnData<'_>) -> Value {
    use serde_json::to_value;

    match data {
        ColumnData::Binary(v) => to_value(v),
        ColumnData::Bit(v) => to_value(v),
        ColumnData::Date(_) => {
            to_value(&NaiveDate::from_sql(&column_to_owned(data)).expect("Date"))
        }
        ColumnData::DateTime(_) => {
            to_value(&NaiveDateTime::from_sql(&column_to_owned(data)).expect("DateTime"))
        }
        ColumnData::DateTime2(_) => {
            to_value(&NaiveDateTime::from_sql(&column_to_owned(data)).expect("DateTime2"))
        }
        ColumnData::DateTimeOffset(_) => to_value(
            &DateTime::<FixedOffset>::from_sql(&column_to_owned(data)).expect("DateTimeOffset"),
        ),
        ColumnData::F32(v) => to_value(v),
        ColumnData::F64(v) => to_value(v),
        ColumnData::Guid(v) => to_value(v),
        ColumnData::I16(v) => to_value(v),
        ColumnData::I32(v) => to_value(v),
        ColumnData::I64(v) => to_value(v),

        #[cfg(feature = "dec19x5")]
        ColumnData::Numeric(_) => {
            to_value(&dec19x5::Decimal::from_sql(&column_to_owned(data)).expect("Numeric"))
        }
        #[cfg(not(feature = "dec19x5"))]
        ColumnData::Numeric(_) => panic!("numeric not supported."),

        ColumnData::SmallDateTime(_) => {
            to_value(&NaiveDateTime::from_sql(&column_to_owned(data)).expect("SmallDateTime"))
        }

        ColumnData::String(v) => to_value(&v),
        ColumnData::Time(_) => {
            to_value(&NaiveTime::from_sql(&column_to_owned(data)).expect("Time"))
        }
        ColumnData::U8(v) => to_value(&v),
        ColumnData::Xml(_) => panic!("xml not supported"),
    }
    .expect("diff")
}
