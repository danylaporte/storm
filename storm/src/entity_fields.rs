use serde::{Deserialize, Serialize};

pub trait EntityFields {
    type Fields;
}

#[derive(Clone, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FieldsOrStr<Fields> {
    Fields(Fields),
    String(String),
}
