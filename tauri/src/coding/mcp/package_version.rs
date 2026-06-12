use std::collections::HashSet;

use futures_util::future::join_all;
use reqwest::Client;
use serde_json::Value;

use super::types::{
    McpPackageVersionManager, McpPackageVersionResolveRequest, McpPackageVersionResolveResult,
};
use crate::{http_client, SqliteDbState};

const NPM_REGISTRY_BASE_URL: &str = "https://registry.npmjs.org";
const PYPI_REGISTRY_BASE_URL: &str = "https://pypi.org/pypi";

pub async fn resolve_package_versions(
    state: &SqliteDbState,
    requests: Vec<McpPackageVersionResolveRequest>,
) -> Vec<McpPackageVersionResolveResult> {
    let unique_requests = unique_requests(requests);
    if unique_requests.is_empty() {
        return Vec::new();
    }

    let client = match http_client::client_with_timeout(state, 10).await {
        Ok(client) => client,
        Err(error) => {
            return unique_requests
                .into_iter()
                .map(|request| request_error_result(request, error.clone()))
                .collect();
        }
    };

    join_all(
        unique_requests
            .into_iter()
            .map(|request| resolve_single_package_version(client.clone(), request)),
    )
    .await
}

async fn resolve_single_package_version(
    client: Client,
    request: McpPackageVersionResolveRequest,
) -> McpPackageVersionResolveResult {
    let version_result = match request.manager {
        McpPackageVersionManager::Npx => {
            fetch_npm_latest_version(&client, &request.package_name).await
        }
        McpPackageVersionManager::Uv => {
            fetch_pypi_latest_version(&client, &request.package_name).await
        }
    };

    match version_result {
        Ok(version) => McpPackageVersionResolveResult {
            manager: request.manager,
            package_name: request.package_name,
            version: Some(version),
            error_message: None,
        },
        Err(error) => request_error_result(request, error),
    }
}

async fn fetch_npm_latest_version(client: &Client, package_name: &str) -> Result<String, String> {
    let package_url = format!(
        "{}/{}",
        NPM_REGISTRY_BASE_URL,
        encode_url_path_segment(package_name)
    );
    let metadata = fetch_registry_json(client, &package_url).await?;
    parse_npm_latest_version(&metadata)
        .ok_or_else(|| format!("No npm latest version found for package '{package_name}'"))
}

async fn fetch_pypi_latest_version(client: &Client, package_name: &str) -> Result<String, String> {
    let distribution_name = strip_python_extras(package_name);
    let package_url = format!(
        "{}/{}/json",
        PYPI_REGISTRY_BASE_URL,
        encode_url_path_segment(&distribution_name)
    );
    let metadata = fetch_registry_json(client, &package_url).await?;
    parse_pypi_latest_version(&metadata)
        .ok_or_else(|| format!("No PyPI version found for package '{distribution_name}'"))
}

async fn fetch_registry_json(client: &Client, url: &str) -> Result<Value, String> {
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, "AI-Toolbox")
        .send()
        .await
        .map_err(|error| format!("Failed to request package registry: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Package registry returned HTTP {}",
            response.status()
        ));
    }

    response
        .json::<Value>()
        .await
        .map_err(|error| format!("Failed to parse package registry response: {error}"))
}

fn unique_requests(
    requests: Vec<McpPackageVersionResolveRequest>,
) -> Vec<McpPackageVersionResolveRequest> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for request in requests {
        let package_name = request.package_name.trim().to_string();
        if package_name.is_empty() {
            continue;
        }

        let key = (request.manager.clone(), package_name.to_lowercase());
        if seen.insert(key) {
            unique.push(McpPackageVersionResolveRequest {
                manager: request.manager,
                package_name,
            });
        }
    }

    unique
}

fn request_error_result(
    request: McpPackageVersionResolveRequest,
    error: String,
) -> McpPackageVersionResolveResult {
    McpPackageVersionResolveResult {
        manager: request.manager,
        package_name: request.package_name,
        version: None,
        error_message: Some(error),
    }
}

fn parse_npm_latest_version(metadata: &Value) -> Option<String> {
    metadata
        .get("dist-tags")
        .and_then(|dist_tags| dist_tags.get("latest"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn parse_pypi_latest_version(metadata: &Value) -> Option<String> {
    metadata
        .get("info")
        .and_then(|info| info.get("version"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn strip_python_extras(package_name: &str) -> String {
    package_name
        .split_once('[')
        .map(|(name, _)| name)
        .unwrap_or(package_name)
        .to_string()
}

fn encode_url_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn parse_npm_latest_version_reads_dist_tag() {
        let metadata = serde_json::json!({
            "dist-tags": {
                "latest": "1.2.3"
            }
        });

        assert_eq!(parse_npm_latest_version(&metadata), Some("1.2.3".to_string()));
    }

    #[test]
    fn parse_pypi_latest_version_reads_info_version() {
        let metadata = serde_json::json!({
            "info": {
                "version": "2026.1.0"
            }
        });

        assert_eq!(
            parse_pypi_latest_version(&metadata),
            Some("2026.1.0".to_string())
        );
    }

    #[test]
    fn encode_url_path_segment_escapes_scoped_npm_package_names() {
        assert_eq!(
            encode_url_path_segment("@modelcontextprotocol/server-time"),
            "%40modelcontextprotocol%2Fserver-time"
        );
    }

    #[test]
    fn strip_python_extras_keeps_distribution_name_only() {
        assert_eq!(strip_python_extras("mcp-server-fetch[cli]"), "mcp-server-fetch");
    }

    #[test]
    fn unique_requests_deduplicates_by_manager_and_package_name() {
        let requests = vec![
            McpPackageVersionResolveRequest {
                manager: McpPackageVersionManager::Npx,
                package_name: "ai-search-mcp".to_string(),
            },
            McpPackageVersionResolveRequest {
                manager: McpPackageVersionManager::Npx,
                package_name: " AI-SEARCH-MCP ".to_string(),
            },
            McpPackageVersionResolveRequest {
                manager: McpPackageVersionManager::Uv,
                package_name: "ai-search-mcp".to_string(),
            },
        ];

        let unique = unique_requests(requests);
        assert_eq!(unique.len(), 2);

        let counts = unique
            .into_iter()
            .fold(HashMap::new(), |mut acc, request| {
                *acc.entry(request.manager).or_insert(0) += 1;
                acc
            });
        assert_eq!(counts.get(&McpPackageVersionManager::Npx), Some(&1));
        assert_eq!(counts.get(&McpPackageVersionManager::Uv), Some(&1));
    }
}
