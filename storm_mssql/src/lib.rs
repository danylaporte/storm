mod client_factory;
mod conn_pool;
mod execute;
mod query;

pub use client_factory::ClientFactory;
pub use execute::Execute;
pub use query::Query;

pub type Client = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;
