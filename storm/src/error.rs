use std::{
    fmt::{self, Debug, Display},
    mem::{replace, swap},
    ops::{Add, AddAssign},
};

pub enum Error {
    AlreadyInTransaction,
    AsyncCellLock(async_cell_lock::Error),
    ClientInError,
    ColumnNull,
    ConvertFailed(String),
    EntityNotFound,
    Internal,
    NotInTransaction,
    ProviderNotFound,
    Std(StdError),
    Str(&'static str),
    String(String),

    #[cfg(feature = "mssql")]
    Mssql(tiberius::error::Error),

    Other(Box<dyn std::error::Error + Send + Sync>),
    Others(Vec<Error>),
}

impl Error {
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: std::error::Error + 'static,
    {
        match self {
            Self::Other(v) => v.downcast_ref(),
            _ => None,
        }
    }

    #[cfg(feature = "mssql")]
    pub fn as_mssql(&self) -> Option<&tiberius::error::Error> {
        match self {
            Self::Mssql(e) => Some(e),
            _ => None,
        }
    }

    pub fn std<E: Into<StdError>>(e: E) -> Self {
        Self::Std(e.into())
    }
}

impl Add for Error {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.add_assign(rhs);
        self
    }
}

impl AddAssign for Error {
    fn add_assign(&mut self, rhs: Self) {
        match (self, rhs) {
            (Self::Others(l), Self::Others(mut r)) => {
                if r.capacity() > l.capacity() {
                    swap(l, &mut r);
                }

                l.extend(r);
            }
            (Self::Others(l), r) => l.push(r),
            (l, Self::Others(mut r)) => {
                r.push(replace(l, Self::Internal));
                *l = Self::Others(r);
            }
            (l, r) => *l = Self::Others(vec![replace(l, Self::Internal), r]),
        }
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AsyncCellLock(e) => Debug::fmt(e, f),
            Self::Std(e) => Debug::fmt(e, f),
            Self::Str(e) => write!(f, "storm::Error::Str({e})"),
            Self::String(e) => write!(f, "storm::Error::Str({e})"),
            Self::Other(e) => Debug::fmt(e, f),
            Self::Others(e) => Debug::fmt(e, f),

            #[cfg(feature = "mssql")]
            Self::Mssql(e) => Debug::fmt(e, f),

            e => Display::fmt(e, f),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyInTransaction => f.write_str("Already in transaction."),
            Self::AsyncCellLock(e) => Display::fmt(e, f),
            Self::ClientInError => f.write_str("Client in error state."),
            Self::ColumnNull => f.write_str("Column is null."),
            Self::ConvertFailed(s) => f.write_str(&format!("Convert failed: `{s}`")),
            Self::EntityNotFound => f.write_str("Entity not found."),
            Self::Internal => f.write_str("Internal."),
            Self::Other(e) => Display::fmt(e, f),
            Self::Others(e) => {
                let mut found = false;

                e.iter()
                    .map(|s| {
                        if found {
                            write!(f, "\n{s}")
                        } else {
                            found = true;
                            write!(f, "{s}")
                        }
                    })
                    .collect()
            }
            Self::NotInTransaction => f.write_str("Not in transaction."),
            Self::ProviderNotFound => f.write_str("Provider not found."),

            #[cfg(feature = "mssql")]
            Self::Mssql(e) => Display::fmt(e, f),

            Self::Str(e) => Display::fmt(e, f),
            Self::String(e) => Display::fmt(e, f),
            Self::Std(e) => Display::fmt(e, f),
        }
    }
}

impl std::error::Error for Error {}

impl From<async_cell_lock::Error> for Error {
    fn from(e: async_cell_lock::Error) -> Self {
        Error::AsyncCellLock(e)
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(b: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Std(b)
    }
}

#[cfg(feature = "mssql")]
impl From<tiberius::error::Error> for Error {
    fn from(e: tiberius::error::Error) -> Self {
        Error::Mssql(e)
    }
}

type StdError = Box<dyn std::error::Error + Send + Sync>;
