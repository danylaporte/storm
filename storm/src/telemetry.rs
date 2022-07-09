use metrics::{counter, gauge};
use std::time::Instant;

pub fn inc_storm_gc() {
    counter!("storm.gc", 1);
}

pub fn inc_storm_cache_island_gc() {
    counter!("storm.cache.island.gc", 1);
}

pub fn inc_storm_execute_time(instant: Instant, op: &'static str, ty: &'static str) {
    counter!("storm.execute.count", 1, "op" => op, "type" => ty);
    counter!("storm.execute.time", instant.elapsed().as_nanos() as u64, "op" => op, "type" => ty);
}

pub fn update_storm_table_rows(mut len: usize, ty: &'static str) {
    if len > u32::MAX as usize {
        len = u32::MAX as usize;
    }

    gauge!("storm.table.rows", len as f64, "type" => ty);
}
