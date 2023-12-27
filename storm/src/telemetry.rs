use metrics::{counter, gauge};

pub fn inc_storm_gc() {
    counter!("storm.gc").increment(1);
}

pub fn inc_storm_cache_island_gc() {
    counter!("storm.cache.island.gc").increment(1);
}

pub fn update_storm_table_rows(mut len: usize, ty: &'static str) {
    if len > u32::MAX as usize {
        len = u32::MAX as usize;
    }

    gauge!("storm.table.rows", "type" => ty).set(len as f64);
}
