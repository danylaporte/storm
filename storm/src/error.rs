use crate::Fields;
use std::{
    fmt::{self, Debug, Display},
    mem::{replace, swap},
};

/// Formats a key for an error message.
///
/// Not generic, so the formatting is compiled once instead of once per entity or constraint.
#[cold]
#[inline(never)]
pub(crate) fn debug_string(v: &dyn Debug) -> String {
    format!("{v:?}")
}

pub enum Error {
    AlreadyInTransaction,
    AsyncCellLock(async_cell_lock::Error),
    ClientInError,
    ColumnNull,
    ConvertFailed(String),
    EntityNotFound,
    FieldTooLong {
        len: usize,
        max: usize,
        field: Box<dyn Fields>,
    },
    /// The parent referenced by a foreign key does not exist.
    ForeignKeyNotFound {
        entity: &'static str,
        field: &'static str,
        parent: &'static str,
        key: String,
        /// The field in the entity fields enum, `None` when the entity has no fields enum.
        entity_field: Option<Box<dyn Fields>>,
    },
    /// The parent cannot be removed because it is still referenced (`on_delete = "restrict"`).
    ForeignKeyRestricted {
        entity: &'static str,
        field: &'static str,
        parent: &'static str,
        key: String,
        count: usize,
    },
    Internal,
    Multiple(Vec<Error>),
    NotInTransaction,
    ProviderNotFound,
    /// A business rule blocks the removal (see [on_remove_rule](crate::on_remove_rule)).
    RemoveRestricted {
        entity: &'static str,
        key: String,
        reason: String,
    },
    TransactionError,
    Std(StdError),
    Str(&'static str),
    /// Another row already has the same value(s) in a unique index.
    UniqueViolation {
        entity: &'static str,
        index: &'static str,
        fields: &'static [&'static str],
        value: String,
        key: String,
        /// The fields in the entity fields enum, empty when the entity has no fields enum.
        entity_fields: Vec<Box<dyn Fields>>,
    },
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
            Self::EntityNotFound => f.write_str("Entity not found."),
            Self::FieldTooLong { len, max, field } => {
                write!(f, "{field} field too long, len: {len}, max {max}")
            }
            Self::ForeignKeyNotFound {
                entity,
                field,
                parent,
                key,
                ..
            } => write!(
                f,
                "Foreign key violation: {entity}.{field} references {parent} `{key}` which does not exist."
            ),
            Self::ForeignKeyRestricted {
                entity,
                field,
                parent,
                key,
                count,
            } => write!(
                f,
                "Foreign key violation: {parent} `{key}` is referenced by {count} {entity} through `{field}`."
            ),
            Self::Multiple(vec) => match &vec[..] {
                [e] => Display::fmt(&e, f),
                _ => f.write_str("Multiple errors"),
            },
            Self::TransactionError => f.write_str("Transaction error."),
            Self::Internal => f.write_str("Internal."),
            Self::NotInTransaction => f.write_str("Not in transaction."),
            Self::ProviderNotFound => f.write_str("Provider not found."),
            Self::RemoveRestricted {
                entity,
                key,
                reason,
            } => write!(f, "{entity} `{key}` cannot be removed: {reason}"),

            #[cfg(feature = "mssql")]
            Self::Mssql(e) => Display::fmt(e, f),

            Self::Str(e) => Display::fmt(e, f),
            Self::UniqueViolation {
                entity,
                index,
                fields,
                value,
                key,
                ..
            } => write!(
                f,
                "Unique index violation: {entity} `{index}` ({}) value {value} is already used by `{key}`.",
                fields.join(", ")
            ),
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
