use crate::{Execute, Parameter, Result};
use tiberius::ToSql;

pub struct UpsertBuilder<'a> {
    insert_fields: String,
    insert_values: String,
    params: Vec<Parameter<'a>>,
    update_setters: String,
    update_wheres: String,
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

    pub fn add_field_owned<T: ToSql>(&mut self, name: &str, value: T) {
        self.params.push(Parameter::from_owned(value));
        self.add_field(name);
    }

    pub fn add_field_ref<T: ToSql>(&mut self, name: &str, value: &'a T) {
        self.params.push(Parameter::from_ref(value));
        self.add_field(name);
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

    fn param(&self) -> String {
        format!("@p{}", self.params.len())
    }

    pub fn sql(&self) -> String {
        format!(
            "
            BEGIN TRY
                INSERT INTO {table} ({insert_fields}) VALUES ({insert_values});
            END TRY
            BEGIN CATCH
                IF ERROR_NUMBER() IN (2601, 2627)
                    UPDATE {table} SET {update_setters} WHERE {update_wheres};
            END CATCH
            ",
            table = self.table,
            insert_fields = self.insert_fields,
            insert_values = self.insert_values,
            update_setters = self.update_setters,
            update_wheres = self.update_wheres,
        )
    }
}
