use crate::{Error, Execute, FromSql, Parameter, QueryRows, Result, ToSql};
use smallvec::SmallVec;
use std::fmt::Write;
use storm::IsDefined;
use tiberius::ColumnData;
use tracing::error;

pub struct UpsertBuilder<'a> {
    insert_fields: String,
    insert_values: String,
    params: Vec<Parameter<'a>>,
    update_setters: String,
    update_wheres: String,
    upsert_mode: UpsertMode,
    table: &'a str,
}

impl<'a> UpsertBuilder<'a> {
    pub fn new(table: &'a str) -> Self {
        Self {
            insert_fields: String::new(),
            insert_values: String::new(),
            params: Vec::new(),
            update_setters: String::new(),
            update_wheres: String::new(),
            upsert_mode: UpsertMode::InsertThanUpdate,
            table,
        }
    }

    fn add_field(&mut self, name: &str) {
        if !self.insert_fields.is_empty() {
            self.insert_fields.push(',');
            self.insert_values.push(',');
            self.update_setters.push(',');
        }

        let param = self.params.len();

        self.insert_fields.push_str(name);
        push_param(&mut self.insert_values, param);

        self.update_setters.push_str(name);
        self.update_setters.push('=');
        push_param(&mut self.update_setters, param);
    }

    pub fn add_field_identity<T: IsDefined + ToSql>(&mut self, name: &str, value: T) {
        if value.is_defined() {
            self.upsert_mode = UpsertMode::Update;
            self.params.push(Parameter::from_owned(value));
            self.add_field(name);
        } else {
            self.upsert_mode = UpsertMode::Insert;
        }
    }

    pub fn add_field_owned<T: ToSql>(&mut self, name: &str, value: T) {
        self.params.push(Parameter::from_owned(value));
        self.add_field(name);
    }

    pub fn add_field_ref<T: ToSql>(&mut self, name: &str, value: &'a T) {
        self.params.push(Parameter::from_ref(value));
        self.add_field(name);
    }

    pub fn add_key_identity<T: IsDefined + ToSql>(&mut self, name: &str, value: T) {
        if value.is_defined() {
            self.upsert_mode = UpsertMode::Update;
        } else {
            self.upsert_mode = UpsertMode::Insert;
        }

        self.params.push(Parameter::from_owned(value));

        if !self.update_wheres.is_empty() {
            self.update_wheres.push_str("AND");
        }

        self.add_wheres(name, self.params.len());
    }

    pub fn add_key_ref<T: ToSql>(&mut self, name: &str, value: &'a T) {
        self.params.push(Parameter::from_ref(value));

        if !self.insert_fields.is_empty() {
            self.insert_fields.push(',');
            self.insert_values.push(',');
        }

        if !self.update_wheres.is_empty() {
            self.update_wheres.push_str("AND");
        }

        let param = self.params.len();

        self.insert_fields.push_str(name);
        push_param(&mut self.insert_values, param);

        self.add_wheres(name, param);
    }

    fn add_wheres(&mut self, name: &str, param: usize) {
        self.update_wheres.push('(');
        self.update_wheres.push_str(name);
        self.update_wheres.push('=');
        push_param(&mut self.update_wheres, param);
        self.update_wheres.push(')');
    }

    pub async fn execute<P: Execute>(self, provider: &P) -> Result<()> {
        let sql = self.sql();
        let params = self.param_refs();
        provider.execute(sql, &params).await?;
        Ok(())
    }

    pub async fn execute_identity<K, P>(self, provider: &P, key: &mut K) -> Result<()>
    where
        K: for<'b> FromSql<'b> + ToSql + Send,
        P: Execute + QueryRows,
    {
        let sql = self.sql();
        let params = self.param_refs();

        provider.execute(sql, &params).await?;

        if self.upsert_mode == UpsertMode::Insert {
            let cast_ty = column_data_to_sql_type(key.to_sql())?;

            let one: OneValue<K> = provider
                .query_rows(
                    format!("SELECT CAST(@@IDENTITY as {cast_ty})"),
                    &[],
                    |row| K::from_sql(row.get(0)),
                    true,
                )
                .await?;

            *key = one.0.ok_or(storm::Error::EntityNotFound)?;
        }

        Ok(())
    }

    fn param_refs(&self) -> SmallVec<[&dyn ToSql; 16]> {
        self.params.iter().map(|v| v as _).collect()
    }

    fn push_insert_sql(&self, sql: &mut String) {
        if self.insert_fields.is_empty() {
            // when there is no fields in the table except an identity column.
            let _ = write!(sql, "INSERT INTO {} DEFAULT VALUES", self.table);
        } else {
            let _ = write!(
                sql,
                "INSERT INTO {} ({}) VALUES ({})",
                self.table, self.insert_fields, self.insert_values
            );
        }
    }

    pub fn sql(&self) -> String {
        let mut sql = String::with_capacity(
            self.table.len() * 2
                + self.insert_fields.len()
                + self.insert_values.len()
                + self.update_setters.len()
                + self.update_wheres.len()
                + 160,
        );

        match self.upsert_mode {
            UpsertMode::Insert => self.push_insert_sql(&mut sql),
            UpsertMode::InsertThanUpdate => {
                if self.update_setters.is_empty() {
                    let _ = write!(
                        sql,
                        "IF NOT EXISTS(SELECT 1 FROM {} WHERE {}) ",
                        self.table, self.update_wheres
                    );
                    self.push_insert_sql(&mut sql);
                    sql.push(';');
                } else {
                    sql.push_str("\n                        ");
                    self.push_update_sql(&mut sql);
                    sql.push_str("\n                        IF @@ROWCOUNT = 0\n                        BEGIN\n                            ");
                    self.push_insert_sql(&mut sql);
                    sql.push_str("\n                        END\n                    ");
                }
            }
            UpsertMode::Update => self.push_update_sql(&mut sql),
        }

        sql
    }

    fn push_update_sql(&self, sql: &mut String) {
        if !self.update_setters.is_empty() {
            let _ = write!(
                sql,
                "UPDATE {} SET {} WHERE {}",
                self.table, self.update_setters, self.update_wheres
            );
        }
    }
}

// fmt::Write for String is infallible.
fn push_param(sql: &mut String, index: usize) {
    let _ = write!(sql, "@p{index}");
}

struct OneValue<T>(Option<T>);

impl<T> Default for OneValue<T> {
    fn default() -> Self {
        OneValue(None)
    }
}

impl<T> Extend<T> for OneValue<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        if self.0.is_none() {
            self.0 = iter.into_iter().next();
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum UpsertMode {
    InsertThanUpdate,
    Insert,
    Update,
}

fn column_data_to_sql_type(data: ColumnData<'_>) -> Result<&'static str> {
    match data {
        ColumnData::I16(_) => Ok("smallint"),
        ColumnData::I32(_) => Ok("int"),
        ColumnData::I64(_) => Ok("bigint"),
        ColumnData::U8(_) => Ok("tinyint"),
        _ => {
            error!("key type is not supported as identity.");
            Err(Error::Internal)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UpsertBuilder;

    #[test]
    fn insert_than_update_sql() {
        let id = 1_i32;
        let a = 2_i32;
        let b = 3_i32;
        let mut builder = UpsertBuilder::new("[T]");
        builder.add_field_ref("[A]", &a);
        builder.add_field_ref("[B]", &b);
        builder.add_key_ref("[Id]", &id);

        assert_eq!(
            builder.sql(),
            "
                        UPDATE [T] SET [A]=@p1,[B]=@p2 WHERE ([Id]=@p3)
                        IF @@ROWCOUNT = 0
                        BEGIN
                            INSERT INTO [T] ([A],[B],[Id]) VALUES (@p1,@p2,@p3)
                        END
                    "
        );
    }

    #[test]
    fn insert_if_not_exists_sql() {
        let id = 1_i32;
        let mut builder = UpsertBuilder::new("[T]");
        builder.add_key_ref("[Id]", &id);

        assert_eq!(
            builder.sql(),
            "IF NOT EXISTS(SELECT 1 FROM [T] WHERE ([Id]=@p1)) INSERT INTO [T] ([Id]) VALUES (@p1);"
        );
    }

    #[test]
    fn identity_insert_and_update_sql() {
        let a = 2_i32;

        let mut builder = UpsertBuilder::new("[T]");
        builder.add_field_ref("[A]", &a);
        builder.add_key_identity("[Id]", 0_i32);
        assert_eq!(builder.sql(), "INSERT INTO [T] ([A]) VALUES (@p1)");

        let mut builder = UpsertBuilder::new("[T]");
        builder.add_key_identity("[Id]", 0_i32);
        assert_eq!(builder.sql(), "INSERT INTO [T] DEFAULT VALUES");

        let mut builder = UpsertBuilder::new("[T]");
        builder.add_field_ref("[A]", &a);
        builder.add_key_identity("[Id]", 7_i32);
        assert_eq!(builder.sql(), "UPDATE [T] SET [A]=@p1 WHERE ([Id]=@p2)");
    }
}
