use crate::ClearEvent;

pub trait Clearable {
    fn cleared() -> &'static ClearEvent;
}
