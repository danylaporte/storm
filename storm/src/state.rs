use std::fmt::{self, Debug, Formatter};

pub enum LogState<T> {
    Inserted(T),
    Removed,
}

impl<T: Clone> Clone for LogState<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Inserted(v) => Self::Inserted(v.clone()),
            Self::Removed => Self::Removed,
        }
    }
}

impl<T: Debug> Debug for LogState<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inserted(v) => f.debug_tuple("Inserted").field(v).finish(),
            Self::Removed => f.write_str("Removed"),
        }
    }
}

impl<T: PartialEq> PartialEq for LogState<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Inserted(l), Self::Inserted(r)) => l == r,
            (Self::Removed, Self::Removed) => true,
            _ => false,
        }
    }
}
