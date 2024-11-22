use crate::Fields;
use std::{
    fmt::{self, Debug, Display},
    mem::{replace, swap},
};

pub enum Error {
    AlreadyInTransaction,
    AsyncCellLock(async_cell_lock::Error),
    ClientInError,
    ColumnNull,
    ConvertFailed(String),
    CycleDepInit(&'static str),
    EntityNotFound,
    FieldTooLong {
        len: usize,
        max: usize,
        field: Box<dyn Fields>,
    },
    Internal,
    Multiple(Vec<Error>),
    NotInTransaction,
    ProviderNotFound,
    TransactionError,
    Std(StdError),
    Str(&'static str),
    String(String),

    #[cfg(feature = "mssql")]
    Mssql(tiberius::error::Error),
}

impl Error {
    pub fn downcast<T>(self) -> Result<Box<T>, Self>
    where
        T: std::error::Error + 'static,
    {
        match self {
            Self::Std(v) => v
                .downcast()
                .or_else(|v| v.downcast().map(|v| *v))
                .map_err(Self::Std),
            v => Err(v),
        }
    }

    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: std::error::Error + 'static,
    {
        match self {
            Self::Std(v) => v.downcast_ref(),
            _ => None,
        }
    }

    pub fn extend_one(&mut self, other: Error) {
        match (self, other) {
            (Self::Multiple(a), Self::Multiple(mut b)) => {
                // optimize to extend the biggest which should
                // minimize the allocation here.
                if a.len() < b.len() {
                    swap(a, &mut b);
                }

                a.extend(b);
            }
            (Self::Multiple(a), b) => a.push(b),
            (a, Self::Multiple(mut b)) => {
                b.push(replace(a, Self::ColumnNull));
                *a = Self::Multiple(b)
            }
            (a, b) => *a = Self::Multiple(vec![replace(a, Self::ColumnNull), b]),
        }
    }

    pub(crate) fn extend_one_opt(this: &mut Option<Self>, other: Self) {
        match this {
            Some(e) => e.extend_one(other),
            None => *this = Some(other),
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

impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AsyncCellLock(e) => Debug::fmt(e, f),
            Self::Std(e) => Debug::fmt(e, f),
            Self::Str(e) => write!(f, "storm::Error::Str({e})"),
            Self::String(e) => write!(f, "storm::Error::Str({e})"),

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
            Self::ConvertFailed(s) => write!(f, "Convert failed: `{s}`"),
            Self::CycleDepInit(s) => write!(f, "Cylcle dependency init `{s}`"),
            Self::EntityNotFound => f.write_str("Entity not found."),
            Self::FieldTooLong { len, max, field } => {
                write!(f, "{field} field too long, len: {len}, max {max}")
            }
            Self::Multiple(vec) => match &vec[..] {
                [e] => Display::fmt(&e, f),
                _ => f.write_str("Multiple errors"),
            },
            Self::TransactionError => f.write_str("Transaction error."),
            Self::Internal => f.write_str("Internal."),
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

#[test]
fn check_downcast() {
    use std::fmt::{self, Debug, Display, Formatter};
    struct MyErr;

    impl Debug for MyErr {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("MyErr")
        }
    }

    impl Display for MyErr {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            f.write_str("MyErr")
        }
    }

    impl std::error::Error for MyErr {}

    let e = Error::std(Box::new(MyErr));

    e.downcast::<MyErr>().unwrap();
}
