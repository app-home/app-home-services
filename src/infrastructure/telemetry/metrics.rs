use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// Installs the Prometheus recorder as the global `metrics` backend and returns a
/// handle that renders the current metric snapshot as Prometheus exposition text.
///
/// Call this once, near the start of `main`, before any `metrics::gauge!`/`counter!`
/// calls happen elsewhere in the app (those macros are no-ops until a recorder is
/// installed). The returned handle is what the `/metrics` HTTP route renders on each
/// scrape.
pub fn install_prometheus_recorder() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus metrics recorder")
}
