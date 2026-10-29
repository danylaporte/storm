[![CI](https://github.com/danylaporte/storm/actions/workflows/ci.yml/badge.svg)](https://github.com/danylaporte/storm/actions/workflows/ci.yml)

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
- Foreign keys enforced in memory (restrict, cascade or set null on delete).
- Unique indexes on one or multiple fields, case / accent insensitive, filtered.
- Support a provider model, MSSQL using tiberius is implemented.

## Foreign keys

Annotate a field of an entity deriving `Ctx` to declare a foreign key to another entity:

```rust
#[derive(Ctx, PartialEq /* + provider derives */)]
struct Order {
    #[storm(foreign_key(entity = Customer))] // on_delete = "restrict" by default
    customer_id: u32,

    #[storm(foreign_key(entity = Employee, on_delete = "set_null"))]
    seller_id: Option<u32>,
}

impl Entity for Order {
    type Key = u32;
}
```

- Inserting or updating a row that references a missing parent fails with `Error::ForeignKeyNotFound`.
  `None` values and self references are allowed.
- `on_delete` controls what happens to the children when the parent is removed:
  - `"restrict"` (default): the removal fails with `Error::ForeignKeyRestricted`. Checked after the parent's
    `removing` handlers, so they can remove or detach the children first.
  - `"cascade"`: the children are removed as well (recursively through their own foreign keys).
  - `"set_null"`: the field is set to `None` and the children are saved. Requires an `Option` field and a `Clone` entity.
- The children of a parent are found with a scan of the child table. A reverse index (`{Entity}{Field}FkIndex`,
  e.g. `OrderCustomerIdFkIndex`) is generated for each foreign key and used instead when declared with
  `index = "Name"` or already built. Once built, it is kept and maintained on every change of the child table.
  It can also be queried directly (`ctx.ref_as::<OrderCustomerIdFkIndex>()`).
  It is a vec-based `FlatSetIndex`: the keys of the child and parent entities must implement `Into<u32>` and
  `TryFrom<u32>`. `None` values of an `Option` field are indexed too (`idx.none()`).
- `hash` generates a `HashFlatSetIndex` instead (slower), for a parent key not based on `u32`,
  e.g. `#[storm(foreign_key(entity = Customer, hash))]`. The key of the child entity must still implement
  `Into<u32>` and `TryFrom<u32>`.
- `no_index` generates no reverse index: the children are always found with a scan of the child table.
  Required when the key of the child entity is not based on `u32` (e.g. a composite key),
  e.g. `#[storm(foreign_key(entity = User, no_index))]`.
- The database may declare `ON DELETE NO ACTION` for a `cascade` or `set_null` foreign key: storm applies the
  action itself before deleting the parent.
- Checks are done in the transaction, before the provider is called.

### Foreign keys to a parent in another crate

When the parent is defined in a crate depending on the child crate, the child can't name it: declare the
foreign key with the `table` of the parent instead of `entity`, and register it on the parent with
`referenced_by`:

```rust
// child crate
#[derive(Ctx, PartialEq /* + provider derives */)]
struct WorkItem {
    #[storm(foreign_key(table = "[dbo].[ProcessActions]", on_delete = "set_null"))]
    process_action_id: Option<u32>,
}

// parent crate, depending on the child crate
#[derive(Ctx, PartialEq /* + provider derives */)]
#[storm(referenced_by = child_crate::WorkItemProcessActionIdFk)]
struct ProcessAction { /* ... */ }
```

- The child generates the marker (`{Entity}{Field}Fk`, implementing `storm::foreign_key::ExternalForeignKey`)
  and the reverse index. Same options as with `entity` (`on_delete`, `index`, `hash`, `no_index`).
- The parent enforces it as the `ForeignKey` `storm::foreign_key::External<Marker, Parent>`: the key of the
  parent must be the type of the field (without `Option`). Without `referenced_by`, only the schema validation
  uses it.
- `table` is compared with the referenced table by the schema validation.

### Foreign keys enforced by the database only

`schema_only` declares the foreign key for the schema validation only: no check, index or `on_delete` action is
generated, the database enforces it. For entities not saved by storm (ex: without `MssqlSave`), where a `restrict`
would block the removal of the parent and `set_null` can't save the child:

```rust
#[storm(foreign_key(table = "[dbo].[Files]", on_delete = "set_null", schema_only))]
file_id: Option<u32>,
```

- Works with `entity` or `table`, incompatible with `index`, `flat_index`, `hash` and `no_index`.
- The field type is not used: `set_null` also accepts a wrapper of an `Option` (ex: `EqTrue<Option<u32>>`).
- On the type, `foreign_key(column = ..., schema_only)` declares a foreign key on a column not mapped to a field
  (ex: a column loaded by a lazy part only), with `entity` or `table` and without `key`:

```rust
#[storm(foreign_key(column = "EncryptKeyId", table = "[dbo].[EncryptKeys]", schema_only))]
struct Post { /* the encrypt key id is in a lazy part */ }
```

### Foreign keys on key columns

A column of the key can reference a parent with `foreign_key(column = ...)` on the type:

```rust
#[derive(Ctx, PartialEq /* + provider derives */)]
#[storm(foreign_key(column = "CustomerId", entity = Customer, on_delete = "cascade"))]
struct CustomerSettings { /* ... */ }

impl Entity for CustomerSettings {
    type Key = u32; // the customer id
}

#[derive(Ctx, PartialEq /* + provider derives */)]
#[storm(collection = "hash_table", foreign_key(column = "GroupId", entity = Group, key = 0))]
struct GroupPermission;

impl Entity for GroupPermission {
    type Key = (u32, u32); // (GroupId, PermissionId)
}
```

- `column` is the database column, used to name the marker (`{Entity}{Column}Fk`) and by the schema validation.
- `key` is the position of the column in a composite (tuple) key. Without it, the whole key references the
  parent and must have the type of the parent key.
- Same rules as the foreign keys on fields, except `"set_null"` which is not supported.
- No reverse index is generated: the child is found by key for a single key, with a scan of the child table
  for a composite key.

## Unique indexes

Declare a unique index on a field, or on multiple fields at the entity level:

```rust
#[derive(Ctx, PartialEq /* + provider derives */)]
#[storm(unique(fields = "company_id, code"))]
#[storm(unique(name = "active_login", fields = "login", filter = "is_active", collation = "ai_ci"))]
struct Employee {
    #[storm(unique)]
    email: String,

    company_id: u32,
    code: Option<String>,
    login: String,
    active: bool,
}

fn is_active(e: &Employee) -> bool {
    e.active
}
```

- Inserting or updating a row with the same value(s) as another row fails with `Error::UniqueViolation`.
- `collation` controls how strings are compared (using `str_utils`, enabled by the default `str_utils` feature):
  `"ci"` (default, case insensitive), `"ai"` (accent insensitive), `"ai_ci"` (case and accent insensitive)
  or `"exact"`.
- `None` is a value: only one row can have `None` for the same key. Use `filter` to exclude rows, like a
  SQL filtered index. It's a function or closure taking `&Entity` and returning `bool`
  (e.g. `filter = |e: &Employee| e.code.is_some()`).
- `name` defaults to the field names joined with `_`. A marker (`{Entity}{Name}Unique`, implementing
  `storm::Unique`) and an index (`{Entity}{Name}UniqueIndex`, e.g. `EmployeeCompanyIdCodeUniqueIndex`) are generated.
  Use `storm::unique::find::<EmployeeEmailUnique>(&mut trx, &key)` to look up a row in a transaction.
- Field types must implement `storm::UniqueValue` (provided for strings, integers, `bool`, `char`, `Uuid`, chrono types,
  `Decimal`, `Option`, `Box` and `Arc`). Use `storm::unique_value!(MyType)` for custom `Clone + Debug + Eq + Hash` types.
  As for foreign keys, the key of the entity must implement `Into<u32>` and `TryFrom<u32>`.
- `no_index` generates no index: the duplicates are found with a scan of the table. Required when the key of the
  entity is not based on `u32` (e.g. a composite key), e.g. `#[storm(unique(no_index))]`.
- Rows are checked one at a time, so swapping values between two rows requires an intermediate value.
- Values generated by the provider (e.g. identity columns) are not checked.

## Field length

`#[storm(max_length = 50)]` on a `MssqlSave` field rejects values that are too long with `Error::FieldTooLong`
before anything is sent to the database (every field too long is reported). It also generates a
`{FIELD}_MAX_LENGTH` const and is verified by the generated schema test (see below).

- Strings are measured in UTF-16 code units, like SQL Server does for `nvarchar(n)`: a character outside the
  basic multilingual plane (ex: an emoji) counts for 2. This also matches `varchar(n)` with a single-byte code page,
  but not a `varchar` with a UTF-8 collation, which counts bytes.
- `Translated` fields are checked for each culture of `Culture::DB_CULTURES`.
- Other types must implement `storm::Len` (provided for strings, `Vec`, slices, `Option`, `Box`, `Arc`, `Cow`,
  references, `StringCi` and `FormStr`).

## Schema validation

`MssqlLoad` implements `storm_mssql::MssqlSchema` (the table, keys, columns, unique indexes and foreign keys
declared on the model) and, unless `no_test`, generates a `test_{entity}_entity` test that connects with the `DB`
environment variable and compares the model to the database. The comparison is also available with
`storm_mssql::check_schema(&provider, &Entity::SCHEMA)`, which returns the list of mismatches.

- Every mapped column must exist (also in the translated table).
- Partial entities (`partial = true`) also compare the nullability: an `Option` field requires a nullable column and
  a nullable column requires an `Option` field. Use `#[storm(nullable = true)]` on a field whose type maps `NULL`
  itself.
- A bounded `char`, `varchar`, `nchar`, `nvarchar`, `binary` or `varbinary` column requires a `max_length` equal
  to its size (in characters for `nchar`/`nvarchar`, in bytes otherwise); a `(max)`, `text` or non-text column
  must not have one.
- Every unique index of the database must be declared on the model with the same columns (in any order),
  collation (`_BIN` or `_CS_AS` = `exact`, `_CI_AS` = `ci`, `_CS_AI` = `ai`, `_CI_AI` = `ai_ci`) and filter (filtered or
  not; the filter expression itself is not compared). Indexes containing all the key columns are ignored.
- Every single-column foreign key of the database must be declared on the model, referencing the same table, with
  the same `on_delete` (`ON DELETE SET DEFAULT` is not supported). Composite foreign keys are ignored.
- Unique indexes and foreign keys declared only on the model are allowed.
- The parent of a `foreign_key` must implement `storm_mssql::MssqlMeta` (ex: derive `MssqlLoad`), unless declared
  with `table`.
- The error messages tell which attribute to add, change or remove.

