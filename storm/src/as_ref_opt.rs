pub trait AsRefOpt<T> {
    fn as_ref_opt(&self) -> Option<&T>;
}

pub trait FromRefOpt<U> {
    fn from_ref_opt(u: &U) -> Option<&Self>;
}

impl<T, U> FromRefOpt<U> for T
where
    U: AsRefOpt<T>,
{
    fn from_ref_opt(u: &U) -> Option<&Self> {
        u.as_ref_opt()
    }
}
