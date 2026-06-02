use tracing_subscriber::EnvFilter;

/// Build the tracing filter. `level` is the global default applied to every
/// `loopal-*` crate. Using a global default (rather than a per-crate
/// whitelist) ensures new crates are never silently dropped from logging —
/// the prior whitelist had omitted the tui/session/hub/view-state crates,
/// making frontend/IPC failures invisible. Known-noisy third-party crates are
/// pinned lower so the default `debug` stays readable.
pub fn build_env_filter(level: &str) -> EnvFilter {
    EnvFilter::new(filter_directives(level))
}

fn filter_directives(level: &str) -> String {
    format!(
        "{level},hyper=info,hyper_util=info,h2=info,rustls=info,reqwest=info,\
         tokio=info,tokio_util=info,mio=info,want=info,rusqlite=info,rmcp=info,\
         html5ever=info,selectors=info"
    )
}

#[cfg(test)]
mod tests {
    use super::filter_directives;

    #[test]
    fn default_level_is_global_not_per_crate() {
        let d = filter_directives("debug");
        assert!(
            d.starts_with("debug,"),
            "global default level must lead the directive list: {d}"
        );
    }

    #[test]
    fn noisy_third_party_crates_are_pinned() {
        let d = filter_directives("debug");
        for noisy in ["hyper=info", "rustls=info", "tokio=info", "rmcp=info"] {
            assert!(d.contains(noisy), "missing third-party pin {noisy}: {d}");
        }
    }

    #[test]
    fn no_loopal_crate_is_whitelisted() {
        // Regression: a per-crate whitelist is what dropped frontend crates.
        // The global-default design must not reintroduce explicit loopal pins.
        assert!(!filter_directives("debug").contains("loopal_"));
    }
}
