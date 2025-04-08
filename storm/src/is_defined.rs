/// Indicate if a key is not zero. This trait must be implemented on keys
/// used by the UpsertMut when a key is flagged as Identity (MS SQL).
/// In such a case, the provider will check that property to
/// determine if the entity must be inserted or updated.
pub trait IsDefined {
    fn is_defined(&self) -> bool;
}

impl<T> IsDefined for Option<T>
where
    T: IsDefined,
{
    fn is_defined(&self) -> bool {
        self.as_ref().is_some_and(IsDefined::is_defined)
    }
}

macro_rules! is_defined {
    ($t:ty) => {
        impl IsDefined for $t {
            fn is_defined(&self) -> bool {
                *self != 0
            }
        }
    };
}

is_defined!(i16);
is_defined!(i32);
is_defined!(i64);
is_defined!(i8);
is_defined!(isize);
is_defined!(u16);
is_defined!(u32);
is_defined!(u64);
is_defined!(u8);
is_defined!(usize);
