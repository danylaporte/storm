use crate::TouchedEvent;

pub trait Touchable {
    fn touched() -> &'static TouchedEvent;
}
