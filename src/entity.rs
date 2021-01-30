use crate::Row;

pub trait Entity {
    type Key;
    type Row: Row;
}
