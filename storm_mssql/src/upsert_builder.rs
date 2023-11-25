use crate::{Error, Execute, FromSql, Parameter, QueryRows, ToSql};
use storm::{IsDefined, Result};
use tiberius::ColumnData;
use tracing::error;

pub struct UpsertBuilder<'a> {
    insert_fields: String,
    insert_values: String,
    params: Vec<Parameter<'a>>,
    update_setters: String,
    update_wheres: String,
    upsert_mode: UpsertMode,
    table: &'static str,
}

impl<'a> UpsertBuilder<'a> {
    pub fn new(table: &'static str) -> Self {
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

        let param = &self.param();

        self.insert_fields.push_str(name);
        self.insert_values.push_str(param);

        self.update_setters.push_str(name);
        self.update_setters.push('=');
        self.update_setters.push_str(param);
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

        let param = &self.param();

        self.add_wheres(name, param);
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

        let param = &self.param();

        self.insert_fields.push_str(name);
        self.insert_values.push_str(param);

        self.add_wheres(name, param);
    }

    fn add_wheres(&mut self, name: &str, param: &str) {
        self.update_wheres.push('(');
        self.update_wheres.push_str(name);
        self.update_wheres.push('=');
        self.update_wheres.push_str(param);
        self.update_wheres.push(')');
    }

    pub async fn execute<P: Execute>(self, provider: &P) -> Result<()> {
        let sql = self.sql();
        let params = self.params.iter().map(|v| v as _).collect::<Vec<_>>();
        provider.execute(sql, params.as_slice()).await?;
        Ok(())
    }

    pub async fn execute_identity<K, P>(self, provider: &P, key: &mut K) -> Result<()>
    where
        K: for<'b> FromSql<'b> + ToSql + Send,
        P: Execute + QueryRows,
    {
        let sql = self.sql();
        let params = self.params.iter().map(|v| v as _).collect::<Vec<_>>();

        provider.execute(sql, params.as_slice()).await?;

        if self.upsert_mode == UpsertMode::Insert {
            let cast_ty = column_data_to_sql_type(key.to_sql(), self.table)?;

            let mut rows: Vec<K> = provider
                .query_rows(
                    format!("SELECT CAST(@@IDENTITY as {cast_ty})"),
                    &[],
                    |row| {
                        K::from_sql(row.get(0)).map_err(|error| {
                            storm::Error::from(Error::FromSql {
                                column: "@@IDENTITY",
                                error,
                                table: self.table,
                                ty: std::any::type_name::<K>(),
                            })
                        })
                    },
                    true,
                )
                .await?;

            *key = rows.pop().ok_or(Error::FetchIdentify)?;
        }

        Ok(())
    }

    fn insert_sql(&self) -> String {
        if self.insert_fields.is_empty() {
            // when there is no fields in the table except an identity column.
            format!("INSERT INTO {} DEFAULT VALUES", self.table)
        } else {
            format!(
                "INSERT INTO {} ({}) VALUES ({})",
                self.table, self.insert_fields, self.insert_values
            )
        }
    }

    fn param(&self) -> String {
        format!("@p{}", self.params.len())
    }

    pub fn sql(&self) -> String {
        match self.upsert_mode {
            UpsertMode::Insert => self.insert_sql(),
            UpsertMode::InsertThanUpdate => {
                let update = self.update_sql();
                let insert = self.insert_sql();

                if update.is_empty() {
                    format!(
                        "
                        BEGIN TRY
                        {insert}
                        END TRY
                        BEGIN CATCH
                            IF ERROR_NUMBER() NOT IN (2601, 2627) THROW;
                        END CATCH
                        "
                    )
                } else {
                    format!(
                        "
                        {update}
                        IF @@ROWCOUNT = 0
                        BEGIN
                            BEGIN TRY
                            {insert}
                            END TRY
                            BEGIN CATCH
                                IF ERROR_NUMBER() IN (2601, 2627)
                                BEGIN
                                {update}
                                END
                                ELSE
                                    THROW;
                            END CATCH
                        END
                    "
                    )
                }
            }
            UpsertMode::Update => self.update_sql(),
        }
    }

    fn update_sql(&self) -> String {
        if self.update_setters.is_empty() {
            String::new()
        } else {
            format!(
                "UPDATE {} SET {} WHERE {}",
                self.table, self.update_setters, self.update_wheres
            )
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum UpsertMode {
    InsertThanUpdate,
    Insert,
    Update,
}

fn column_data_to_sql_type(data: ColumnData<'_>, table: &'static str) -> Result<&'static str> {
    match data {
        ColumnData::I16(_) => Ok("smallint"),
        ColumnData::I32(_) => Ok("int"),
        ColumnData::I64(_) => Ok("bigint"),
        ColumnData::U8(_) => Ok("tinyint"),
        _ => {
            error!("key type is not supported as identity.");
            Err(crate::Error::IdentityType { table }.into())
        }
    }
}
