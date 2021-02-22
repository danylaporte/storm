mod apply_log;
mod ctx_types;
mod entity;
mod error;
mod get;
mod get_or_load;
mod init;
mod map_transaction;
pub mod mem;
pub mod provider;
mod state;
mod trx_cell;
mod version;

pub use apply_log::ApplyLog;
pub use ctx_types::CtxTypes;
pub use entity::Entity;
pub use error::Error;
pub use get::Get;
pub use get_or_load::GetOrLoad;
pub use init::Init;
pub use map_transaction::MapTransaction;
pub use once_cell::sync::OnceCell;
use state::State;
pub use trx_cell::TrxCell;
pub use version::Version;

type Log<E> = fxhash::FxHashMap<<E as Entity>::Key, State<E>>;
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "derive")]
pub use storm_derive::Ctx;

#[cfg(feature = "mssql")]
pub use storm_derive::{MssqlLoad, MssqlSave};
