use std::{env::VarError, fmt::Display, io};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed connecting to sql server named instance; {0}")]
    ConnectNamed(#[source] tiberius::error::Error),

    #[error("failed creating client; {0}")]
    CreateClient(#[source] tiberius::error::Error),

    #[error("entity not found on table {table}")]
    EntityNotFound { table: &'static str },

    #[error("failed to query @@identity")]
    FetchIdentify,

    #[error("field diff error on {table}.{column}; {source}")]
    FieldDiff {
        column: &'static str,
        source: FieldDiffError,
        table: &'static str,
    },

    #[error("from_sql error on {table}.{column}; {source}")]
    FromSql {
        column: &'static str,
        source: FromSqlError,
        table: &'static str,
        ty: &'static str,
    },

    #[error("identity type not supported on {table}")]
    IdentityType { table: &'static str },

    #[error("mssql io: {0}")]
    Io(io::Error),

    #[error("failed parse ado connection string; {0}")]
    ParseAdoConnStr(#[source] tiberius::error::Error),

    #[error("query failed; {source}; sql: {sql}")]
    Query {
        source: tiberius::error::Error,
        sql: String,
    },

    #[error(transparent)]
    Unknown(Box<dyn std::error::Error + Send + Sync>),

    #[error("failed accessing env var {name}; {source}")]
    Var { name: String, source: VarError },
}

impl Error {
    pub(crate) fn unknown<E: std::error::Error + Send + Sync + 'static>(error: E) -> Self {
        Self::Unknown(Box::new(error))
    }
}

impl From<Error> for storm::Error {
    fn from(value: Error) -> Self {
        storm::Error::Std(Box::new(value))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FieldDiffError {
    #[error("failed deserialize json, {0}")]
    DeJson(#[source] serde_json::Error),

    #[error("failed serialize json, {0}")]
    SerJson(#[source] serde_json::Error),

    #[error(transparent)]
    Unknown(Box<dyn std::error::Error + Send + Sync>),
}

impl FieldDiffError {
    pub fn unknown<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        Self::Unknown(Box::new(e))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FromSqlError {
    #[error("column is null")]
    ColumnNull,

    #[error("{0}")]
    Custom(String),

    #[error("failed deserialize json, {0}")]
    DeJson(#[source] serde_json::Error),

    #[error("unexpected {0}")]
    Unexpected(String),

    #[error(transparent)]
    Unknown(Box<dyn std::error::Error + Send + Sync>),

    #[error("value: {value}, {source}")]
    UnknownWithVal {
        source: Box<dyn std::error::Error + Send + Sync>,
        value: String,
    },
}

impl FromSqlError {
    pub fn unexpected<V: Display>(value: V) -> Self {
        Self::Unexpected(value.to_string())
    }

    pub fn unknown<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        Self::Unknown(Box::new(e))
    }

    pub fn unknown_with_val<E, V>(source: E, val: V) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
        V: Display,
    {
        Self::UnknownWithVal {
            source: Box::new(source),
            value: val.to_string(),
        }
    }
}
