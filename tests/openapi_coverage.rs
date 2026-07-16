use std::collections::{HashMap, HashSet};

use utoipa::OpenApi;

use app_home_services::adapters::inbound::api_doc::ApiDoc;

/// The authoritative set of (method, path) pairs that must appear in the spec
/// per `data-model.md` and the operational-endpoint policy (FR-015).
const DOCUMENTED_PATH_METHODS: &[(&str, &str)] = &[
    ("POST", "/api/auth/login/password"),
    ("POST", "/api/auth/login/google"),
    ("POST", "/api/auth/logout"),
    ("POST", "/api/auth/refresh"),
    ("GET", "/api/health"),
];

fn spec_paths() -> Vec<(String, Vec<String>)> {
    let spec = ApiDoc::openapi();
    let mut entries = Vec::new();
    for (path, item) in &spec.paths.paths {
        let mut methods = Vec::new();
        if item.post.is_some() {
            methods.push("POST".to_string());
        }
        if item.get.is_some() {
            methods.push("GET".to_string());
        }
        entries.push((path.clone(), methods));
    }
    entries
}

#[test]
fn coverage_matches_expected_set() {
    let paths = spec_paths();
    let spec_paths: HashMap<&str, Vec<String>> =
        paths.iter().map(|(p, m)| (p.as_str(), m.clone())).collect();

    for (method, path) in DOCUMENTED_PATH_METHODS {
        let found = spec_paths
            .get(path)
            .map(|methods| methods.iter().any(|m| m == method));
        assert!(
            found == Some(true),
            "Missing documented endpoint: {method} {path}"
        );
    }

    assert!(
        !spec_paths.contains_key("/metrics"),
        "/metrics should be excluded from the spec"
    );
}

#[test]
fn coverage_fails_on_undocumented_endpoint() {
    let paths = spec_paths();
    let spec_paths: HashSet<&str> = paths.iter().map(|(p, _)| p.as_str()).collect();

    for (method, path) in DOCUMENTED_PATH_METHODS {
        assert!(
            spec_paths.contains(path),
            "Coverage failure: {method} {path} is not in the generated spec"
        );
    }
}
