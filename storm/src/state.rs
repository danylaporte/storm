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
