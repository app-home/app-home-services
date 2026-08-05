use axum::Router;

/// Applies the HTTP security headers emitted on every response (see #90).
///
/// Emitted unconditionally: HSTS is inert over plain HTTP (browsers only process
/// it on HTTPS responses), so sending it is safe even when TLS is terminated by
/// a reverse proxy ahead of this service. CSP is deliberately NOT set: this
/// service renders no HTML except the Swagger UI when `ENABLE_SWAGGER=true`
/// (which relies on inline scripts/CDN assets), so a strict CSP would break it
/// without adding value.
pub fn apply_security_headers<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            axum::http::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::time::Duration;

    use axum::Json;
    use axum::http::StatusCode;
    use axum::routing::get;
    use rcgen::CertifiedKey;

    /// Exercises the native-TLS serving stack end to end, mirroring the branch
    /// in `main.rs` (see #93): installs the rustls crypto provider, loads the
    /// PEM files through `RustlsConfig::from_pem_file`, serves via
    /// `axum_server::bind_rustls` and performs a real HTTPS request. This runs
    /// in CI (no database required) and catches TLS-only startup/handshake
    /// regressions that plain-HTTP integration tests would miss.
    #[tokio::test]
    async fn native_tls_smoke_test() {
        // rustls 0.23 does not auto-select a provider when both aws-lc-rs and
        // ring are compiled in (as in this graph). Production installs
        // aws-lc-rs in main(); this test installs the same provider and ignores
        // the error if a previous test in this binary already did so.
        let _ = rustls::crypto::CryptoProvider::install_default(
            rustls::crypto::aws_lc_rs::default_provider(),
        );

        let CertifiedKey { cert, key_pair } =
            rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
                .expect("failed to generate self-signed cert");

        let dir = std::env::temp_dir().join(format!("tls-smoke-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("failed to create temp cert dir");
        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");
        std::fs::write(&cert_path, cert.pem()).expect("failed to write cert PEM");
        std::fs::write(&key_path, key_pair.serialize_pem()).expect("failed to write key PEM");

        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .expect("failed to load test TLS config");

        // Grab a free ephemeral port by binding a throwaway listener (axum-server
        // then binds internally, avoiding the from_std listener path that stalls
        // connection handling on Windows -- see specs/012-native-tls/plan.md).
        let probe = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("probe bind failed");
        let addr: SocketAddr = probe.local_addr().expect("probe local_addr failed");
        drop(probe);

        let app = super::apply_security_headers(axum::Router::new().route(
            "/api/health",
            get(|| async { (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))) }),
        ));
        let service = app.into_make_service_with_connect_info::<SocketAddr>();
        let server = axum_server::bind_rustls(addr, tls_config);
        let handle = tokio::spawn(async move {
            server.serve(service).await.expect("server error");
        });

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("failed to build reqwest client");

        let url = format!("https://127.0.0.1:{}/api/health", addr.port());
        let mut response = None;
        for _ in 0..20 {
            match client.get(&url).send().await {
                Ok(resp) => {
                    response = Some(resp);
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        let response = response.expect("native TLS server did not become ready");

        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert_eq!(
            headers
                .get("strict-transport-security")
                .and_then(|v| v.to_str().ok()),
            Some("max-age=31536000; includeSubDomains")
        );
        assert_eq!(
            headers
                .get("x-content-type-options")
                .and_then(|v| v.to_str().ok()),
            Some("nosniff")
        );
        assert_eq!(
            headers.get("x-frame-options").and_then(|v| v.to_str().ok()),
            Some("DENY")
        );
        assert_eq!(
            headers.get("referrer-policy").and_then(|v| v.to_str().ok()),
            Some("strict-origin-when-cross-origin")
        );

        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
