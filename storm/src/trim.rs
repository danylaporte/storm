//! `#[storm(trim)]`: string fields trimmed when the entity is inserted or updated, before the
//! `upserting` event.
//!
//! Unique indexes on a trimmed field also trim their keys, so rows loaded with whitespace
//! still match.

use std::sync::Arc;

/// Removes the leading and trailing whitespace of a value, in place.
pub trait TrimInPlace {
    fn trim_in_place(&mut self);
}

impl TrimInPlace for String {
    fn trim_in_place(&mut self) {
        let end = self.trim_end().len();
        self.truncate(end);

        let start = self.len() - self.trim_start().len();
        self.drain(..start);
    }
}

impl TrimInPlace for Box<str> {
    fn trim_in_place(&mut self) {
        if self.trim().len() != self.len() {
            let trimmed: Box<str> = self.trim().into();
            *self = trimmed;
        }
    }
}

impl TrimInPlace for Arc<str> {
    fn trim_in_place(&mut self) {
        if self.trim().len() != self.len() {
            let trimmed: Arc<str> = self.trim().into();
            *self = trimmed;
        }
    }
}

impl<T: TrimInPlace> TrimInPlace for Option<T> {
    #[inline]
    fn trim_in_place(&mut self) {
        if let Some(v) = self {
            v.trim_in_place();
        }
    }
}
