mod entity;
mod entity_delete;
mod entity_load;
mod entity_upsert;
mod load_table;
mod opts_transaction;
mod table;
mod table_appy_log;
mod table_container;
mod table_get;
mod table_log;
mod table_transaction;

pub use entity::Entity;
pub use entity_delete::EntityDelete;
pub use entity_load::EntityLoad;
pub use entity_upsert::EntityUpsert;
pub use load_table::LoadTable;
pub use opts_transaction::OptsTransaction;
pub use table::Table;
pub use table_appy_log::TableAppyLog;
pub use table_container::TableContainer;
pub use table_get::TableGet;
pub use table_log::TableLog;
pub use table_transaction::TableTransaction;

#[cfg(feature = "derive")]
pub use storm_derive::Ctx;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
