#[macro_use]
mod version_deps;

mod apply_log;
mod as_ref_async;
mod as_ref_opt;
mod commit;
mod connected;
mod entity;
mod error;
mod get;
mod get_mut;
mod get_or_load;
mod get_or_load_async;
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
pub use as_ref_async::AsRefAsync;
pub use as_ref_opt::AsRefOpt;
pub use async_cell_lock::{AsyncOnceCell, QueueRwLock};
pub use async_trait;
#[cfg(feature = "cache")]
pub use cache;
pub use commit::Commit;
pub use connected::{Connected, ConnectedRef, ConnectedTrx, ConnectedTrxRef};
pub use entity::Entity;
pub use error::Error;
pub use get::Get;
pub use get_mut::GetMut;
pub use get_or_load::GetOrLoad;
pub use get_or_load_async::GetOrLoadAsync;
pub use get_version::{GetVersion, GetVersionOpt};
pub use init::Init;
pub use insert::Insert;
pub use map_transaction::MapTransaction;
#[cfg(feature = "metrics")]
pub use metrics;
pub use once_cell::sync::OnceCell;
pub use provider::ProviderContainer;
pub use remove::Remove;
use state::State;
pub use transaction::Transaction;
pub use trx_cell::TrxCell;
#[cfg(feature = "vec-map")]
pub use vec_map;
#[cfg(feature = "vec-map")]
pub use vec_map::VecMap;
pub use version::Version;

type Log<E> = fxhash::FxHashMap<<E as Entity>::Key, State<E>>;
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "derive")]
pub use storm_derive::{indexing, Ctx};
#[cfg(feature = "mssql")]
pub use storm_derive::{MssqlLoad, MssqlSave};
pub use storm_derive::{NoopDelete, NoopLoad, NoopSave};

#[cfg(feature = "metrics")]
pub fn register_metrics() {
    use metrics::{register_histogram, Unit};

    register_histogram!(
        "storm.execution_time",
        Unit::Seconds,
        "execution time of a storm request."
    );
}
