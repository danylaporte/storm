# storm

An in memory, database agnostic ORM written in rust.

## Work in progress

### Completed

- Transaction log for pushing changes into the database and in memory.
- Tables are loaded in memory with different strategy and table storage for fast access.
- Can be used with a Read -> Queue -> Write lock model for maximum concurrency.
- Delete, Load, Save are async.
- Tables can be versionnized to detect changes.
- LRU Cache support is provided.
- Partial entity loading / saving.
- Automatic indexing.
- Support a provider model, MSSQL using tiberius is implemented.

