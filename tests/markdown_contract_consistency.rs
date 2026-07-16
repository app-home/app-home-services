use std::collections::HashSet;

use utoipa::OpenApi;

use app_home_services::adapters::inbound::api_doc::ApiDoc;

#[derive(Debug)]
struct ContractEndpoint {
    method: String,
    path: String,
    status_codes: Vec<u16>,
}

fn parse_contract(path: &std::path::Path) -> Option<ContractEndpoint> {
    let content = std::fs::read_to_string(path).ok()?;

    let title_line = content.lines().next()?;
    let (method, path) = if let Some(rest) = title_line.strip_prefix("# Contract: ") {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let is_valid_method = matches!(
            parts.first().copied(),
            Some("GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS")
        );
        if parts.len() == 2 && is_valid_method {
            (parts[0].to_uppercase(), parts[1].to_string())
        } else {
            content
                .lines()
                .skip_while(|l| l.trim() != "## Endpoint")
                .nth(1)
                .and_then(|l| {
                    let trimmed = l.trim().trim_matches('`');
                    let mut parts = trimmed.splitn(2, ' ');
                    let m = parts.next()?.to_uppercase();
                    let p = parts.next()?.to_string();
                    Some((m, p))
                })?
        }
    } else {
        return None;
    };

    let mut status_codes: Vec<u16> = content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("## Response ") {
                let rest = trimmed.trim_start_matches("## Response ");
                let code_str = rest.split_whitespace().next()?;
                code_str.parse::<u16>().ok()
            } else {
                None
            }
        })
        .collect();
    status_codes.sort();

    Some(ContractEndpoint {
        method,
        path,
        status_codes,
    })
}

fn spec_paths() -> Vec<(String, Vec<String>, Vec<u16>)> {
    let spec = ApiDoc::openapi();
    let mut entries = Vec::new();
    for (path, item) in &spec.paths.paths {
        let mut methods = Vec::new();
        let mut status_codes: Vec<u16> = Vec::new();
        if let Some(op) = &item.post {
            methods.push("POST".to_string());
            status_codes.extend(
                op.responses
                    .responses
                    .keys()
                    .filter_map(|k| k.parse::<u16>().ok()),
            );
        }
        if let Some(op) = &item.get {
            methods.push("GET".to_string());
            status_codes.extend(
                op.responses
                    .responses
                    .keys()
                    .filter_map(|k| k.parse::<u16>().ok()),
            );
        }
        status_codes.sort();
        entries.push((path.clone(), methods, status_codes));
    }
    entries
}

fn collect_contracts() -> Vec<ContractEndpoint> {
    let mut contracts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(std::path::Path::new("specs")) {
        for entry in entries.flatten() {
            let contracts_dir = entry.path().join("contracts");
            if contracts_dir.is_dir()
                && let Ok(files) = std::fs::read_dir(&contracts_dir)
            {
                for file in files.flatten() {
                    let p = file.path();
                    if p.extension().map(|e| e == "md").unwrap_or(false)
                        && p.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s != "openapi-doc")
                            .unwrap_or(true)
                        && let Some(ep) = parse_contract(&p)
                    {
                        contracts.push(ep);
                    }
                }
            }
        }
    }
    contracts
}

fn collect_contract_set() -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(std::path::Path::new("specs")) {
        for entry in entries.flatten() {
            let contracts_dir = entry.path().join("contracts");
            if contracts_dir.is_dir()
                && let Ok(files) = std::fs::read_dir(&contracts_dir)
            {
                for file in files.flatten() {
                    let p = file.path();
                    if p.extension().map(|e| e == "md").unwrap_or(false)
                        && p.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s != "openapi-doc")
                            .unwrap_or(true)
                        && let Some(ep) = parse_contract(&p)
                    {
                        set.insert(format!("{} {}", ep.method, ep.path));
                    }
                }
            }
        }
    }
    set
}

#[test]
fn markdown_documented_set_present_glob() {
    let contracts = collect_contracts();
    assert!(!contracts.is_empty(), "No Markdown contract files found");

    let spec_entries = spec_paths();

    for contract in &contracts {
        let matched_spec = spec_entries.iter().find(|(p, methods, _)| {
            p == &contract.path && methods.iter().any(|m| m == &contract.method)
        });

        assert!(
            matched_spec.is_some(),
            "Contract endpoint {} {} is not in the generated spec",
            contract.method,
            contract.path
        );

        let (_, _, spec_codes) = matched_spec.unwrap();
        for code in &contract.status_codes {
            assert!(
                spec_codes.contains(code),
                "Status code {code} for {} {} is in contract but not in spec",
                contract.method,
                contract.path
            );
        }
    }
}

#[test]
fn spec_no_extra_endpoints_beyond_contracts() {
    let contract_paths = collect_contract_set();

    let spec_entries = spec_paths();
    for (path, methods, _) in &spec_entries {
        for method in methods {
            let key = format!("{method} {path}");
            assert!(
                contract_paths.contains(&key) || key == "GET /api/health",
                "Spec endpoint '{}' has no corresponding contract file \
                 — if intentional, add a contract or exclude per FR-015",
                key
            );
        }
    }
}
