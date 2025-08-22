use std::time::Instant;

/// Internal: For use within macros.
#[doc(hidden)]
#[allow(unused_variables)]
pub fn debug_index_get_or_init_elapsed(instant: Instant, name: &str) {
    #[cfg(feature = "telemetry")]
    {
        let elapsed = instant.elapsed().as_millis();

        if elapsed > 250 {
            tracing::warn!(
                elapsed_ms = elapsed,
                name = name,
                "Index get_or_init blocked for too long"
            );
        }
    }
}

/// Internal: For use within macros.
#[doc(hidden)]
#[allow(unused_variables)]
pub fn debug_locks_await_elapsed(instant: Instant) {
    #[cfg(feature = "telemetry")]
    {
        let elapsed = instant.elapsed().as_millis();

        if elapsed > 250 {
            tracing::warn!(elapsed_ms = elapsed, "Locks Await took too long");
        }
    }
}
