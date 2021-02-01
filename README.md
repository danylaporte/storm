# storm

An in memory, database agnostic ORM written in rust.

## Work in progress

### Already or near completed feature

- Transaction log for pushing changes into the database and in memory.
- Tables are loaded in memory with different strategy and table storage for fast access.
- Can be used with a Read -> Queue -> Write lock model for maximum concurrency.
- Loading / saving are async.
- Add versionning support of rows / tables.

### Roadmap

- Add indexing
- Add support for cache.
- Implement basic conn pool for Postgresql.
