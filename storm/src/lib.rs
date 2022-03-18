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
mod error;
pub mod gc;
mod get;
mod get_mut;
mod hash_table;
mod init;
mod insert;
mod is_defined;
pub mod mem;
mod one_to_many;
pub mod prelude;
pub mod provider;
mod remove;
mod state;
mod tag;
mod transaction;
mod vec_table;

pub use accessor::*;
pub use apply_log::ApplyLog;
pub use as_ref_async::AsRefAsync;
pub use as_ref_opt::{AsRefOpt, FromRefOpt};
pub use async_cell_lock::{self, AsyncOnceCell, QueueRwLock};
pub use async_try_from::AsyncTryFrom;
pub use attached;
#[cfg(feature = "cache")]
pub use cache;
pub use ctx::*;
pub use ctx_type_info::CtxTypeInfo;
pub use entity::Entity;
pub use entity_fields::{EntityFields, FieldsOrStr};
pub use entity_of::EntityOf;
pub use error::Error;
pub use gc::*;
pub use get::Get;
pub use get_mut::GetMut;
pub use hash_table::HashTable;
pub use init::Init;
pub use insert::*;
pub use is_defined::IsDefined;
#[cfg(feature = "telemetry")]
pub use metrics;
pub use once_cell::sync::OnceCell;
pub use one_to_many::{OneToMany, OneToManyFromIter};
pub use parking_lot;
pub use provider::ProviderContainer;
pub use remove::Remove;
pub use state::LogState;
pub use tag::{NotifyTag, Tag};
pub use tokio;
pub use transaction::Transaction;
pub use vec_map::{self, VecMap};
pub use vec_table::VecTable;
pub use version_tag::{self, VersionTag};

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a + Send>>;
pub type Log<E> = fxhash::FxHashMap<<E as Entity>::Key, LogState<E>>;
pub type Result<T> = std::result::Result<T, Error>;

pub const EV_CREATED: &str = "created";
pub const OBJ_INDEX: &str = "index";
pub const OBJ_TABLE: &str = "table";

#[cfg(feature = "derive")]
pub use storm_derive::{indexing, Ctx, LocksAwait, NoopDelete, NoopLoad, NoopSave};
#[cfg(feature = "mssql")]
pub use storm_derive::{MssqlDelete, MssqlLoad, MssqlSave};

fn register_metrics() {
    #[cfg(feature = "telemetry")]
    {
        use metrics::{describe_counter, describe_gauge, Unit};
        use std::sync::Once;

        static START: Once = Once::new();

        START.call_once(|| {
            describe_gauge!(
                "storm.execute.count",
                Unit::Count,
                "Operation execution time per type."
            );

            describe_gauge!(
                "storm.execute.time",
                Unit::Nanoseconds,
                "Operation execution time per type."
            );

            #[cfg(feature = "cache")]
            {
                describe_counter!(
                    "storm.cache.island.gc",
                    Unit::Count,
                    "cache island collected"
                );
            }

            describe_counter!("storm.gc", Unit::Count, "storm gc run");
            describe_gauge!("storm.table.rows", Unit::Count, "row count of a table");
        });
    }
}
