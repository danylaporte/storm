use std::fmt::{self, Debug, Display};

pub enum Error {
    AlreadyInTransaction,
    NotInTransaction,
    Std(StdError),
}

impl Error {
    pub fn std<E: Into<StdError>>(e: E) -> Self {
        Self::Std(e.into())
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Std(e) => Debug::fmt(e, f),
            e => Display::fmt(e, f),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInTransaction => f.write_str("Already in transaction."),
            Self::NotInTransaction => f.write_str("Not in transaction."),
            Self::Std(e) => Display::fmt(e, f),
        }
    }
}

impl std::error::Error for Error {}

type StdError = Box<dyn std::error::Error + Send + Sync>;
