use crate::{Error, Fields};
use std::{borrow::Cow, sync::Arc};

/// Length of a value as measured against a column `max_length`.
///
/// Strings are measured in UTF-16 code units, like SQL Server does for `nvarchar(n)`:
/// a character outside the basic multilingual plane (ex: an emoji) counts for 2.
pub trait Len {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[inline]
fn str_len(s: &str) -> usize {
    s.encode_utf16().count()
}

impl<T> Len for &T
where
    T: Len + ?Sized,
{
    #[inline]
    fn is_empty(&self) -> bool {
        Len::is_empty(&**self)
    }

    #[inline]
    fn len(&self) -> usize {
        Len::len(&**self)
    }
}

impl<T> Len for [T] {
    #[inline]
    fn len(&self) -> usize {
        <[_]>::len(self)
    }
}

impl<T> Len for Arc<T>
where
    T: Len + ?Sized,
{
    #[inline]
    fn len(&self) -> usize {
        Len::len(&**self)
    }
}

impl<T> Len for Box<T>
where
    T: Len + ?Sized,
{
    #[inline]
    fn len(&self) -> usize {
        Len::len(&**self)
    }
}

impl<T> Len for Cow<'_, T>
where
    T: Len + ToOwned + ?Sized,
{
    #[inline]
    fn len(&self) -> usize {
        Len::len(&**self)
    }
}

impl<T: Len> Len for Option<T> {
    #[inline]
    fn len(&self) -> usize {
        match self {
            Some(v) => Len::len(v),
            None => 0,
        }
    }
}

impl Len for str {
    #[inline]
    fn is_empty(&self) -> bool {
        str::is_empty(self)
    }

    #[inline]
    fn len(&self) -> usize {
        str_len(self)
    }
}

impl Len for String {
    #[inline]
    fn is_empty(&self) -> bool {
        str::is_empty(self)
    }

    #[inline]
    fn len(&self) -> usize {
        str_len(self)
    }
}

impl<T> Len for Vec<T> {
    #[inline]
    fn len(&self) -> usize {
        <[_]>::len(self)
    }
}

#[cfg(feature = "str_utils")]
impl<F> Len for str_utils::form_str::FormStr<F> {
    #[inline]
    fn is_empty(&self) -> bool {
        str::is_empty(self)
    }

    #[inline]
    fn len(&self) -> usize {
        str_len(self)
    }
}

#[cfg(feature = "str_utils")]
impl Len for str_utils::str_ci::StringCi {
    #[inline]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    fn len(&self) -> usize {
        str_len(&self.0)
    }
}

#[doc(hidden)]
pub fn macro_check_max_len(
    len: usize,
    max: usize,
    field: impl Fields + 'static,
    error: &mut Option<Error>,
) {
    if max != 0 && len > max {
        Error::extend_one_opt(
            error,
            Error::FieldTooLong {
                len,
                max,
                field: Box::new(field),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::Len;
    use std::{borrow::Cow, sync::Arc};

    #[test]
    fn str_len_is_utf16_code_units() {
        assert_eq!(Len::len("abc"), 3);
        assert_eq!(Len::len("éè"), 2);
        assert_eq!(Len::len("a😀"), 3);
        assert_eq!(Len::len(&String::from("😀😀")), 4);
        assert_eq!(Len::len(&Box::<str>::from("😀")), 2);
        assert_eq!(Len::len(&Arc::<str>::from("😀")), 2);
        assert_eq!(Len::len(&Cow::Borrowed("😀")), 2);
        assert_eq!(Len::len(&Some("😀")), 2);
        assert_eq!(Len::len(&None::<String>), 0);
        assert_eq!(Len::len(&vec![1u8, 2]), 2);
    }
}
