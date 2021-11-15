use crate::Result;
use serde_json::Value;
use std::{collections::HashMap, hash::BuildHasher};
use storm::{EntityFields, FieldsOrStr};

pub trait ApplyEntityDiff: EntityFields {
    fn apply_entity_diff<S: BuildHasher>(
        &mut self,
        map: &HashMap<FieldsOrStr<Self::Fields>, Value, S>,
    ) -> Result<()>;
}

pub trait EntityDiff: EntityFields {
    fn entity_diff<S: BuildHasher>(&self, old: &Self, map: &mut HashMap<Self::Fields, Value, S>);
}
