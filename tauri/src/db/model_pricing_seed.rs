use serde::Serialize;

use crate::db::SqliteDbState;
use crate::http_client;

pub use ai_toolbox_storage::model_pricing_seed::{
    ensure_seeded, ensure_seeded_from_bundled, ensure_seeded_from_cache, get_model_pricing_cache_path, set_cache_dir,
};
use ai_toolbox_storage::model_pricing_seed::{
    insert_pricing_items, parse_pricing_items, write_cache_file,
};

/// Remote sources for the official price list, tried in order.
///
/// The first entry is canonical; the second is a CDN mirror because
/// raw.githubusercontent.com is unreachable on some networks, where the sync
/// used to fail with nothing but a swallowed error (issue #390).
const MODEL_PRICING_REMOTE_URLS: [&str; 2] = [
    "https://raw.githubusercontent.com/coulsontl/ai-toolbox/main/tauri/resources/model_pricing.json",
    "https://cdn.jsdelivr.net/gh/coulsontl/ai-toolbox@main/tauri/resources/model_pricing.json",
];

/// One remote source the sync touched, in attempt order. `error` is `None` for
/// the source that succeeded, so the caller can tell a primary-source success
/// (`attempts.len() == 1`) from a mirror fallback.
#[derive(Debug, Clone, Serialize)]
pub struct ModelPricingSyncAttempt {
    pub url: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelPricingSyncResult {
    pub inserted_count: usize,
    /// Source that produced the rows, i.e. the last entry of `attempts`.
    pub source_url: String,
    pub attempts: Vec<ModelPricingSyncAttempt>,
}

pub async fn fetch_remote_model_pricing(
    db_state: &SqliteDbState,
) -> Result<ModelPricingSyncResult, String> {
    let client = http_client::client_with_timeout(db_state, 30).await?;
    let mut attempts = Vec::with_capacity(MODEL_PRICING_REMOTE_URLS.len());

    for url in MODEL_PRICING_REMOTE_URLS {
        match fetch_one_source(&client, db_state, url).await {
            Ok(inserted_count) => {
                if !attempts.is_empty() {
                    log::info!(
                        "[ModelPricing] Synced remote pricing from fallback source {}",
                        url
                    );
                }
                attempts.push(ModelPricingSyncAttempt {
                    url: url.to_string(),
                    error: None,
                });
                return Ok(ModelPricingSyncResult {
                    inserted_count,
                    source_url: url.to_string(),
                    attempts,
                });
            }
            Err(error) => {
                log::warn!(
                    "[ModelPricing] Failed to fetch remote pricing from {}: {}",
                    url,
                    error
                );
                attempts.push(ModelPricingSyncAttempt {
                    url: url.to_string(),
                    error: Some(error),
                });
            }
        }
    }

    Err(format!(
        "All remote model pricing sources failed: {}",
        describe_attempts(&attempts)
    ))
}

fn describe_attempts(attempts: &[ModelPricingSyncAttempt]) -> String {
    attempts
        .iter()
        .map(|attempt| {
            format!(
                "{} ({})",
                attempt.url,
                attempt.error.as_deref().unwrap_or("no error recorded")
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

async fn fetch_one_source(
    client: &reqwest::Client,
    db_state: &SqliteDbState,
    url: &str,
) -> Result<usize, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Failed to fetch remote model pricing: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Remote model pricing request failed: {}",
            response.status()
        ));
    }

    let content = response
        .text()
        .await
        .map_err(|error| format!("Failed to read remote model pricing response: {error}"))?;
    let pricing_items = parse_pricing_items(&content, "remote model pricing")?;

    if let Err(error) = write_cache_file(&pricing_items) {
        log::warn!("[ModelPricing] Failed to write cache: {}", error);
    }

    db_state.with_conn(|conn| insert_pricing_items(conn, &pricing_items, "remote model pricing"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_pricing_sources_mirror_the_bundled_price_list() {
        // A single source is what let issue #390 go unnoticed: when the only host
        // is unreachable the sync fails with nothing but a swallowable error.
        assert!(MODEL_PRICING_REMOTE_URLS.len() >= 2);
        for url in MODEL_PRICING_REMOTE_URLS {
            assert!(
                url.starts_with("https://"),
                "{url} must be served over https"
            );
            assert!(
                url.ends_with("/tauri/resources/model_pricing.json"),
                "{url} must mirror the bundled price list path"
            );
        }
        // Canonical host first, CDN mirror second: callers read "more than one
        // attempt" as "the primary source failed".
        assert!(MODEL_PRICING_REMOTE_URLS[0].contains("raw.githubusercontent.com"));
        assert!(MODEL_PRICING_REMOTE_URLS[1].contains("cdn.jsdelivr.net"));
    }
}
