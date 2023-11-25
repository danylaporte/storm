use std::borrow::Cow;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("async cell lock error: {0}")]
    AsyncCellLock(#[from] async_cell_lock::Error),

    #[error("custom {0}")]
    Custom(Cow<'static, str>),

    #[error("load one {ty} not found for key {key}")]
    LoadOneNotFound { key: String, ty: &'static str },

    #[error("invalid type cast to {ty} for provider {name}")]
    InvalidProviderType { name: String, ty: &'static str },

    #[error("not in transaction on provider {provider}")]
    NotInTransaction { provider: String },

    #[error("provider {provider} not found")]
    ProviderNotFound { provider: String },

    #[error(transparent)]
    Unknown(#[from] anyhow::Error),
}

impl Error {
    pub fn custom<S>(s: S) -> Self
    where
        Cow<'static, str>: From<S>,
    {
        Self::Custom(s.into())
    }

    pub fn load_one_not_found<K, E>(key: K) -> Self
    where
        K: std::fmt::Debug,
    {
        Self::LoadOneNotFound {
            key: format!("{key:?}"),
            ty: std::any::type_name::<E>(),
        }
    }

    pub fn unknown<E>(error: E) -> Self
    where
        anyhow::Error: From<E>,
    {
        Self::Unknown(error.into())
    }
}
