//! Lazy OpenTelemetry metric instruments via the global meter.
//!
//! Uses `opentelemetry::global::meter()` so instruments are noops when no
//! MeterProvider is installed (telemetry disabled). When a provider is
//! active, metrics are exported via OTLP with zero additional wiring.

use std::sync::OnceLock;

use opentelemetry::metrics::{Counter, Histogram, Meter, UpDownCounter};

fn meter() -> &'static Meter {
    static METER: OnceLock<Meter> = OnceLock::new();
    METER.get_or_init(|| opentelemetry::global::meter("loopal"))
}

pub fn llm_duration() -> &'static Histogram<f64> {
    static INST: OnceLock<Histogram<f64>> = OnceLock::new();
    INST.get_or_init(|| {
        meter()
            .f64_histogram("gen_ai.client.operation.duration")
            .build()
    })
}

pub fn token_usage() -> &'static Counter<u64> {
    static INST: OnceLock<Counter<u64>> = OnceLock::new();
    INST.get_or_init(|| meter().u64_counter("gen_ai.client.token.usage").build())
}

pub fn tool_duration() -> &'static Histogram<f64> {
    static INST: OnceLock<Histogram<f64>> = OnceLock::new();
    INST.get_or_init(|| meter().f64_histogram("loopal.tool.duration").build())
}

pub fn tool_invocations() -> &'static Counter<u64> {
    static INST: OnceLock<Counter<u64>> = OnceLock::new();
    INST.get_or_init(|| meter().u64_counter("loopal.tool.invocations").build())
}

pub fn turn_duration() -> &'static Histogram<f64> {
    static INST: OnceLock<Histogram<f64>> = OnceLock::new();
    INST.get_or_init(|| meter().f64_histogram("loopal.turn.duration").build())
}

pub fn active_turns() -> &'static UpDownCounter<i64> {
    static INST: OnceLock<UpDownCounter<i64>> = OnceLock::new();
    INST.get_or_init(|| meter().i64_up_down_counter("loopal.turns.active").build())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify all instrument factories work with the default noop meter
    // (no MeterProvider installed). They should return valid noop instruments.

    #[test]
    fn llm_duration_creates() {
        let _ = llm_duration();
    }

    #[test]
    fn token_usage_creates() {
        let _ = token_usage();
    }

    #[test]
    fn tool_duration_creates() {
        let _ = tool_duration();
    }

    #[test]
    fn tool_invocations_creates() {
        let _ = tool_invocations();
    }

    #[test]
    fn turn_duration_creates() {
        let _ = turn_duration();
    }

    #[test]
    fn active_turns_creates() {
        let _ = active_turns();
    }
}
