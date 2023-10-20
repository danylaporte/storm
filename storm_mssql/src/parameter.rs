use crate::ToSql;
use std::borrow::{Borrow, Cow};
use tiberius::ColumnData;

pub struct Parameter<'a>(pub(crate) ColumnData<'a>);

impl<'a> Parameter<'a> {
    pub fn from_ref<T: ToSql>(t: &'a T) -> Self {
        Self(t.to_sql())
    }
}

impl Parameter<'static> {
    pub fn from_owned<T: ToSql>(t: T) -> Self {
        fn cow<T: ?Sized + ToOwned>(o: Option<Cow<'_, T>>) -> Option<Cow<'static, T>> {
            match o {
                Some(Cow::Owned(v)) => Some(Cow::Owned(v)),
                Some(Cow::Borrowed(v)) => Some(Cow::Owned(v.to_owned())),
                None => None,
            }
        }

        Self(match t.to_sql() {
            ColumnData::Binary(v) => ColumnData::Binary(cow(v)),
            ColumnData::Bit(v) => ColumnData::Bit(v),
            ColumnData::Date(v) => ColumnData::Date(v),
            ColumnData::DateTime(v) => ColumnData::DateTime(v),
            ColumnData::DateTime2(v) => ColumnData::DateTime2(v),
            ColumnData::DateTimeOffset(v) => ColumnData::DateTimeOffset(v),
            ColumnData::F32(v) => ColumnData::F32(v),
            ColumnData::F64(v) => ColumnData::F64(v),
            ColumnData::Guid(v) => ColumnData::Guid(v),
            ColumnData::I16(v) => ColumnData::I16(v),
            ColumnData::I32(v) => ColumnData::I32(v),
            ColumnData::I64(v) => ColumnData::I64(v),
            ColumnData::Numeric(v) => ColumnData::Numeric(v),
            ColumnData::SmallDateTime(v) => ColumnData::SmallDateTime(v),
            ColumnData::String(v) => ColumnData::String(cow(v)),
            ColumnData::Time(v) => ColumnData::Time(v),
            ColumnData::U8(v) => ColumnData::U8(v),
            ColumnData::Xml(v) => ColumnData::Xml(cow(v)),
        })
    }
}

impl<'a> ToSql for Parameter<'a> {
    fn to_sql(&self) -> ColumnData<'_> {
        tiberius::ToSql::to_sql(self)
    }
}

impl<'a> tiberius::ToSql for Parameter<'a> {
    fn to_sql(&self) -> ColumnData<'_> {
        fn copy<T: Copy>(o: &Option<T>) -> Option<T> {
            o.as_ref().copied()
        }

        fn cow<'a, T: ?Sized + ToOwned>(o: &'a Option<Cow<'a, T>>) -> Option<Cow<'a, T>> {
            o.as_ref().map(|v| match v {
                Cow::Borrowed(v) => Cow::Borrowed(*v),
                Cow::Owned(v) => Cow::Borrowed(v.borrow()),
            })
        }

        match &self.0 {
            ColumnData::Binary(v) => ColumnData::Binary(cow(v)),
            ColumnData::Bit(v) => ColumnData::Bit(copy(v)),
            ColumnData::Date(v) => ColumnData::Date(copy(v)),
            ColumnData::DateTime(v) => ColumnData::DateTime(copy(v)),
            ColumnData::DateTime2(v) => ColumnData::DateTime2(copy(v)),
            ColumnData::DateTimeOffset(v) => ColumnData::DateTimeOffset(copy(v)),
            ColumnData::F32(v) => ColumnData::F32(copy(v)),
            ColumnData::F64(v) => ColumnData::F64(copy(v)),
            ColumnData::Guid(v) => ColumnData::Guid(copy(v)),
            ColumnData::I16(v) => ColumnData::I16(copy(v)),
            ColumnData::I32(v) => ColumnData::I32(copy(v)),
            ColumnData::I64(v) => ColumnData::I64(copy(v)),
            ColumnData::Numeric(v) => ColumnData::Numeric(copy(v)),
            ColumnData::SmallDateTime(v) => ColumnData::SmallDateTime(copy(v)),
            ColumnData::String(v) => ColumnData::String(cow(v)),
            ColumnData::Time(v) => ColumnData::Time(copy(v)),
            ColumnData::U8(v) => ColumnData::U8(copy(v)),
            ColumnData::Xml(v) => ColumnData::Xml(cow(v)),
        }
    }
}

pub fn into_column_data_static(data: &ColumnData<'_>) -> ColumnData<'static> {
    fn copy<T: Copy>(o: &Option<T>) -> Option<T> {
        o.as_ref().copied()
    }

    fn cow<T: ?Sized + ToOwned>(o: &Option<Cow<T>>) -> Option<Cow<'static, T>>
    where
        T::Owned: Clone,
    {
        match o {
            Some(Cow::Borrowed(v)) => Some(Cow::Owned((*v).to_owned())),
            Some(Cow::Owned(v)) => Some(Cow::Owned(v.clone())),
            None => None,
        }
    }

    match data {
        ColumnData::Binary(v) => ColumnData::Binary(cow(v)),
        ColumnData::Bit(v) => ColumnData::Bit(copy(v)),
        ColumnData::Date(v) => ColumnData::Date(copy(v)),
        ColumnData::DateTime(v) => ColumnData::DateTime(copy(v)),
        ColumnData::DateTime2(v) => ColumnData::DateTime2(copy(v)),
        ColumnData::DateTimeOffset(v) => ColumnData::DateTimeOffset(copy(v)),
        ColumnData::F32(v) => ColumnData::F32(copy(v)),
        ColumnData::F64(v) => ColumnData::F64(copy(v)),
        ColumnData::Guid(v) => ColumnData::Guid(copy(v)),
        ColumnData::I16(v) => ColumnData::I16(copy(v)),
        ColumnData::I32(v) => ColumnData::I32(copy(v)),
        ColumnData::I64(v) => ColumnData::I64(copy(v)),
        ColumnData::Numeric(v) => ColumnData::Numeric(copy(v)),
        ColumnData::SmallDateTime(v) => ColumnData::SmallDateTime(copy(v)),
        ColumnData::String(v) => ColumnData::String(cow(v)),
        ColumnData::Time(v) => ColumnData::Time(copy(v)),
        ColumnData::U8(v) => ColumnData::U8(copy(v)),
        ColumnData::Xml(v) => ColumnData::Xml(cow(v)),
    }
}
