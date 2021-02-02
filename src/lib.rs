mod entities_load;
mod entity;
mod entity_delete;
mod entity_upsert;
mod opts_transaction;
mod opts_version;
mod table;
mod table_appy_log;
mod table_container;
mod table_get;
mod table_load;
mod table_log;
mod table_transaction;
mod version;

pub use entities_load::EntitiesLoad;
pub use entity::Entity;
pub use entity_delete::EntityDelete;
pub use entity_upsert::EntityUpsert;
pub use opts_transaction::OptsTransaction;
pub use opts_version::OptsVersion;
pub use table::Table;
pub use table_appy_log::TableAppyLog;
pub use table_container::TableContainer;
pub use table_get::TableGet;
pub use table_load::TableLoad;
pub use table_log::TableLog;
pub use table_transaction::TableTransaction;
pub use version::Version;

#[cfg(feature = "derive")]
pub use storm_derive::Ctx;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
