use crate::{Error, Fields};
use std::{borrow::Cow, sync::Arc};

pub trait Len {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
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

impl<'a, T> Len for Cow<'a, T>
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
        self.chars().count()
    }
}

impl Len for String {
    #[inline]
    fn is_empty(&self) -> bool {
        str::is_empty(self)
    }

    #[inline]
    fn len(&self) -> usize {
        self.chars().count()
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
        self.chars().count()
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
