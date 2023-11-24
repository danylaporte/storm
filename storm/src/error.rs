use std::borrow::Cow;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("already in transaction for {provider}")]
    AlreadyInTransaction { provider: Cow<'static, str> },

    #[error("async cell lock error: {0}")]
    AsyncCellLock(#[from] async_cell_lock::Error),

    #[error("client is in error on provider {provider}")]
    ClientInError { provider: Cow<'static, str> },

    #[error("column is null")]
    ColumnNull,

    #[error("column {table}.{field} is null on provider {provider}")]
    ColumnNullWithCtx {
        provider: Cow<'static, str>,
        table: Cow<'static, str>,
        field: Cow<'static, str>,
    },

    #[error("convert failed; {desc}")]
    ConvertFailed { desc: Cow<'static, str> },

    #[error("convert failed on {table}.{field}, {desc} on provider {provider}")]
    ConvertFailedWithCtx {
        desc: Cow<'static, str>,
        field: Cow<'static, str>,
        provider: Cow<'static, str>,
        table: Cow<'static, str>,
    },

    #[error("storm custom error, {0}")]
    Custom(Cow<'static, str>),

    #[error("load one entity not found")]
    LoadOneNotFound,

    #[error("invalid provider type cast for provider {provider}")]
    InvalidProviderType { provider: Cow<'static, str> },

    #[error("internal error for {table} on provider {provider}")]
    Internal {
        provider: Cow<'static, str>,
        table: Cow<'static, str>,
    },

    #[error("not in transaction on provider {provider}")]
    NotInTransaction { provider: String },

    #[error("provider {provider} not found")]
    ProviderNotFound { provider: String },

    #[error("{0}")]
    SerdeJson(serde_json::Error),

    #[error("{0}")]
    Std(#[from] StdError),
}

type StdError = Box<dyn std::error::Error + Send + Sync>;

// pub enum Error {
//     AlreadyInTransaction {
//         provider: Cow<'static, str>,
//     },
//     AsyncCellLock(async_cell_lock::Error),
//     ClientInError {
//         provider: Cow<'static, str>,
//     },
//     ColumnNull,
//     ColumnNullWithCtx {

//     }
//     ConvertFailed(Cow<'static, str>),
//     EntityNotFound,
//     Internal,
//     NotInTransaction {
//         provider: Cow<'static, str>,
//     },
//     ProviderNotFound {
//         provider: Cow<'static, str>,
//     },
//     Std(StdError),
//     Str(&'static str),
//     String(String),

//     #[cfg(feature = "mssql")]
//     Mssql(tiberius::error::Error),
// }

// impl ErrorKind {
//     #[cfg(feature = "mssql")]
//     pub fn as_mssql(&self) -> Option<&tiberius::error::Error> {
//         match self {
//             Self::Mssql(e) => Some(e),
//             _ => None,
//         }
//     }

//     pub fn into_error(self) -> Error {
//         Error {
//             field: Cow::Borrowed(""),
//             kind: self,
//             table: Cow::Borrowed(""),
//         }
//     }

//     pub fn std<E: Into<StdError>>(e: E) -> Self {
//         Self::Std(e.into())
//     }
// }

// impl Debug for ErrorKind {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Self::AsyncCellLock(e) => Debug::fmt(e, f),
//             Self::Std(e) => Debug::fmt(e, f),
//             Self::Str(e) => write!(f, "storm::Error::Str({e})"),
//             Self::String(e) => write!(f, "storm::Error::Str({e})"),

//             #[cfg(feature = "mssql")]
//             Self::Mssql(e) => Debug::fmt(e, f),

//             e => Display::fmt(e, f),
//         }
//     }
// }

// impl Display for ErrorKind {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Self::AlreadyInTransaction => f.write_str("Already in transaction."),
//             Self::AsyncCellLock(e) => Display::fmt(e, f),
//             Self::ClientInError => f.write_str("Client in error state."),
//             Self::ColumnNull => f.write_str("Column is null."),
//             Self::ConvertFailed(s) => f.write_str(&format!("Convert failed: `{s}`")),
//             Self::EntityNotFound => f.write_str("Entity not found."),
//             Self::Internal => f.write_str("Internal."),
//             Self::NotInTransaction => f.write_str("Not in transaction."),
//             Self::ProviderNotFound => f.write_str("Provider not found."),

//             #[cfg(feature = "mssql")]
//             Self::Mssql(e) => Display::fmt(e, f),

//             Self::Str(e) => Display::fmt(e, f),
//             Self::String(e) => Display::fmt(e, f),
//             Self::Std(e) => Display::fmt(e, f),
//         }
//     }
// }

// impl From<ErrorKind> for Error {
//     #[inline]
//     fn from(value: ErrorKind) -> Self {
//         value.into_error()
//     }
// }

// pub struct Error {
//     field: Cow<'static, str>,
//     table: Cow<'static, str>,
//     kind: ErrorKind,
// }

// pub enum Op {
//     Delete,
//     Insert,
//     Load,
// }

// impl Error {
//     pub fn with_context(self, context: Cow<'static, str>) -> Self {
//         Self {
//             context,
//             kind: self.kind,
//         }
//     }
// }

// impl Error {
//     #[cfg(feature = "mssql")]
//     pub fn as_mssql(&self) -> Option<&tiberius::error::Error> {
//         self.kind.as_mssql()
//     }

//     pub fn std<E: Into<StdError>>(e: E) -> Self {
//         ErrorKind::std(e).into()
//     }
// }

// impl Debug for Error {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.debug_struct("Error")
//             .field("table", &self.table)
//             .field("field", &self.field)
//             .field("kind", &self.kind)
//             .finish()
//     }
// }

// impl Display for Error {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         fn add(
//             f: &mut fmt::Formatter<'_>,
//             first: &mut bool,
//             field: &str,
//             val: &Cow<'static, str>,
//         ) -> fmt::Result {
//             if !val.is_empty() {
//                 if *first {
//                     *first = false;
//                     f.write_str(" on ")?;
//                 } else {
//                     f.write_str(", ")?;
//                 }

//                 write!(f, "{field}: {val}")?;
//             }

//             Ok(())
//         }

//         Display::fmt(&self.kind, f)?;

//         let mut first = true;

//         add(f, &mut first, "table", &self.table)?;
//         add(f, &mut first, "field", &self.field)?;

//         Ok(())
//     }
// }

// impl std::error::Error for Error {}

// impl From<async_cell_lock::Error> for Error {
//     fn from(e: async_cell_lock::Error) -> Self {
//         ErrorKind::AsyncCellLock(e).into_error()
//     }
// }

// impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
//     fn from(b: Box<dyn std::error::Error + Send + Sync>) -> Self {
//         Self::Std(b)
//     }
// }

// #[cfg(feature = "mssql")]
// impl From<tiberius::error::Error> for Error {
//     fn from(e: tiberius::error::Error) -> Self {
//         Error::Mssql(e)
//     }
// }

// type StdError = Box<dyn std::error::Error + Send + Sync>;
