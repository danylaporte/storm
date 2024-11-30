#[macro_use]
mod version_deps;

mod apply_log;
mod as_ref_async;
mod as_ref_opt;
mod async_try_from;
mod change_state;
mod cow_obj;
mod ctx;
mod ctx_type_info;
mod cycle_dep;
mod entity;
mod entity_fields;
mod entity_validate;
mod error;
mod event;
mod fields;
pub mod gc;
mod get;
mod get_mut;
mod hash_one_many;
mod hash_table;
mod init;
mod insert;
mod is_defined;
mod len;
mod log;
pub mod mem;
mod obj;
mod obj_base;
mod one_to_many;
pub mod prelude;
pub mod provider;
mod remove;
mod state;
mod tag;
#[cfg(feature = "telemetry")]
#[doc(hidden)]
pub mod telemetry;
mod trx;
mod trx_err_gate;
mod vec_one_many;
mod vec_table;

pub use apply_log::ApplyLog;
pub use as_ref_async::AsRefAsync;
pub use as_ref_opt::{AsRefOpt, FromRefOpt};
pub use async_cell_lock::{self, AsyncOnceCell, QueueRwLock};
pub use async_try_from::AsyncTryFrom;
pub use attached;
#[cfg(feature = "cache")]
pub use cache;
pub use change_state::ChangeState;
pub use cow_obj::CowObj;
pub use ctx::*;
pub use ctx_type_info::CtxTypeInfo;
pub use entity::{Entity, EntityObj};
pub use entity_fields::{EntityFields, FieldsOrStr};
pub use entity_validate::*;
pub use error::Error;
pub use event::{change_depth, ChangeEvent, ChangedEvent, ClearObjEvent, RemoveEvent};
pub use fields::Fields;
pub use gc::*;
pub use get::{Get, GetOwned};
pub use get_mut::GetMut;
pub use hash_one_many::HashOneMany;
pub use hash_table::HashTable;
pub use init::Init;
pub use insert::*;
pub use is_defined::IsDefined;
pub use len::{macro_check_max_len, Len};
pub use log::{Log, LogToken, LogVars};
#[cfg(feature = "telemetry")]
pub use metrics;
pub use obj::Obj;
pub use obj_base::ObjBase;
pub use once_cell::sync::OnceCell;
pub use one_to_many::{OneToMany, OneToManyFromIter};
pub use parking_lot;
pub use provider::ProviderContainer;
pub use remove::Remove;
pub use state::LogState;
pub use tag::{NotifyTag, Tag};
pub use tokio;
pub use trx::Trx;
use trx_err_gate::TrxErrGate;
pub use vec_map::{self, VecMap};
pub use vec_one_many::VecOneMany;
pub use vec_table::VecTable;
pub use version_tag::{self, VersionTag};

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a + Send>>;
pub type Result<T> = std::result::Result<T, Error>;

pub const EV_CREATED: &str = "created";
pub const OBJ_INDEX: &str = "index";
pub const OBJ_TABLE: &str = "table";

#[cfg(feature = "derive")]
pub use storm_derive::{index, indexing, Ctx, LocksAwait, NoopDelete, NoopLoad, NoopSave};
#[cfg(feature = "mssql")]
pub use storm_derive::{MssqlDelete, MssqlLoad, MssqlSave};

#[macro_export]
macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return Err(e),
        }
    };
}
