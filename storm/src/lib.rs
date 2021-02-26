mod apply_log;
mod commit;
mod connected;
mod ctx_member;
mod entity;
mod error;
mod get;
mod get_or_load;
mod get_version;
mod init;
mod insert;
mod map_transaction;
pub mod mem;
pub mod prelude;
pub mod provider;
mod remove;
mod state;
mod transaction;
mod trx_cell;
mod version;

pub use apply_log::ApplyLog;
pub use commit::Commit;
pub use connected::Connected;
pub use ctx_member::CtxMember;
pub use entity::Entity;
pub use error::Error;
pub use get::Get;
pub use get_or_load::GetOrLoad;
pub use get_version::GetVersion;
pub use init::Init;
pub use insert::Insert;
pub use map_transaction::MapTransaction;
pub use once_cell::sync::OnceCell;
pub use remove::Remove;
use state::State;
pub use transaction::Transaction;
pub use trx_cell::TrxCell;
pub use version::Version;

type Log<E> = fxhash::FxHashMap<<E as Entity>::Key, State<E>>;
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "derive")]
pub use storm_derive::Ctx;

#[cfg(feature = "mssql")]
pub use storm_derive::{MssqlLoad, MssqlSave};
