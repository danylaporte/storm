use crate::Result;
use serde_json::Value;
use std::{collections::HashMap, hash::BuildHasher};

pub trait ApplyEntityDiff {
    fn apply_entity_diff<S: BuildHasher>(&mut self, map: &HashMap<String, Value, S>) -> Result<()>;
}

pub trait EntityDiff {
    fn entity_diff<S: BuildHasher>(&self, old: &Self, map: &mut HashMap<&'static str, Value, S>);
}
