use crate::{FromSql, ToSql};
use std::{
    any::type_name,
    sync::atomic::{AtomicI64, Ordering::Relaxed},
};
use storm::{Error, Result};
use tiberius::ColumnData;

static NEXT_IDENTITY: AtomicI64 = AtomicI64::new(1);

type MakeColumn = fn(i64) -> Option<ColumnData<'static>>;

/// Emulates a database identity in noop mode: assigns the next value of a process-wide
/// counter when `value` is null or zero.
#[doc(hidden)]
pub fn noop_identity<T>(value: &mut T) -> Result<()>
where
    T: for<'b> FromSql<'b> + ToSql,
{
    let (current, make): (Option<i64>, MakeColumn) = match value.to_sql() {
        ColumnData::I16(v) => (v.map(i64::from), |n| {
            Some(ColumnData::I16(Some(i16::try_from(n).ok()?)))
        }),
        ColumnData::I32(v) => (v.map(i64::from), |n| {
            Some(ColumnData::I32(Some(i32::try_from(n).ok()?)))
        }),
        ColumnData::I64(v) => (v, |n| Some(ColumnData::I64(Some(n)))),
        ColumnData::U8(v) => (v.map(i64::from), |n| {
            Some(ColumnData::U8(Some(u8::try_from(n).ok()?)))
        }),
        _ => return Err(convert_failed::<T>("is not supported as identity")),
    };

    if current.is_some_and(|v| v != 0) {
        return Ok(());
    }

    let data = make(NEXT_IDENTITY.fetch_add(1, Relaxed))
        .ok_or_else(|| convert_failed::<T>("identity counter overflow"))?;

    *value = T::from_sql(tiberius::FromSql::from_sql(&data)?)?;

    Ok(())
}

fn convert_failed<T>(msg: &str) -> Error {
    Error::ConvertFailed(format!("`{}` {msg}", type_name::<T>()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::noop_identity;

    #[test]
    fn assigns_only_when_unset() {
        let mut a = 0_i32;
        let mut b = 0_i64;
        noop_identity(&mut a).unwrap();
        noop_identity(&mut b).unwrap();
        assert_ne!(a, 0);
        assert_ne!(b, 0);
        assert_ne!(i64::from(a), b);

        let mut c = 42_i32;
        noop_identity(&mut c).unwrap();
        assert_eq!(c, 42);

        let mut d: Option<i32> = None;
        noop_identity(&mut d).unwrap();
        assert!(d.is_some_and(|v| v != 0));
    }
}
