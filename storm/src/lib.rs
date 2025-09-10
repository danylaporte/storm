#[macro_use]
mod version_deps;

mod accessor;
mod apply_log;
mod as_ref_async;
mod as_ref_opt;
mod async_try_from;
mod ctx;
mod ctx_type_info;
mod entity;
mod entity_fields;
mod entity_of;
mod entity_validate;
mod error;
mod events;
mod fields;
pub mod gc;
mod get;
mod get_mut;
mod hash_table;
pub mod indexing;
mod is_defined;
mod iterator_ext;
mod len;
mod logs;
pub mod mem;
mod one_to_many;
pub mod prelude;
pub mod provider;
pub mod registry;
mod tag;
#[cfg(feature = "telemetry")]
#[doc(hidden)]
pub mod telemetry;
mod touchable;
mod transaction;
mod trx_err_gate;
pub mod trx_iter;
mod utils;
mod vec_table;

pub use accessor::*;
use apply_log::perform_apply_log;
pub use apply_log::{ApplyLog, ApplyOrder, __register_apply};
pub use as_ref_async::AsRefAsync;
pub use as_ref_opt::{AsRefOpt, FromRefOpt};
pub use async_cell_lock::{self, AsyncOnceCell, QueueRwLock};
pub use async_try_from::AsyncTryFrom;
#[cfg(feature = "cache")]
pub use cache;
pub use ctx::*;
pub use ctx_type_info::CtxTypeInfo;
pub use entity::Entity;
pub use entity_fields::{EntityFields, FieldsOrStr};
pub use entity_of::EntityOf;
pub use entity_validate::EntityValidate;
pub use error::Error;
pub use events::*;
pub use extobj;
pub use fast_set::{self, IntSet};
pub use fields::Fields;
pub use fxhash;
pub use gc::*;
pub use get::Get;
pub use get_mut::GetMut;
pub use hash_table::HashTable;
pub use is_defined::IsDefined;
pub use iterator_ext::*;
pub use len::{macro_check_max_len, Len};
pub use linkme;
pub use logs::{LogOf, Logs};
#[cfg(feature = "telemetry")]
pub use metrics;
pub use once_cell::sync::OnceCell;
pub use one_to_many::{OneToMany, OneToManyFromIter};
pub use parking_lot;
pub use provider::ProviderContainer;
pub use registry::set_date_provider;
pub use tag::{NotifyTag, Tag};
pub use tokio;
pub use touchable::Touchable;
pub use transaction::Transaction;
use trx_err_gate::TrxErrGate;
pub use trx_iter::TrxIter;
pub use utils::*;
pub use vec_map::{self, VecMap};
pub use vec_table::VecTable;
pub use version_tag::{self, VersionTag};

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a + Send>>;
pub type Result<T> = std::result::Result<T, Error>;

pub const EV_CREATED: &str = "created";
pub const OBJ_INDEX: &str = "index";
pub const OBJ_TABLE: &str = "table";

#[cfg(feature = "derive")]
pub use storm_derive::{
    flat_set_index, hash_flat_set_index, indexing, one_index, register, single_set, tree_index,
    Ctx, LocksAwait, NoopDelete, NoopLoad, NoopSave,
};
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
