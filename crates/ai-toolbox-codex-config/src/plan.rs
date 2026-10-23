//! Pure planning for the two explicit Codex context overrides.
//!
//! This module does not resolve profiles, open storage, project files, or claim
//! that a Codex client/model supports the requested context size.
use crate::append_toml_configs;
use toml_edit::{DocumentMut, Item, Value};

const CONTEXT: &str = "model_context_window";
const COMPACT: &str = "model_auto_compact_token_limit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Common,
    Provider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Common,
    Provider,
    Unspecified,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ContextPatch {
    pub model_context_window: Option<u64>,
    pub model_auto_compact_token_limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveOverride {
    pub value: Option<i64>,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPlan {
    /// Candidate master-data text only, never a live config projection.
    pub candidate: String,
    pub changed: bool,
    pub context: EffectiveOverride,
    pub auto_compact: EffectiveOverride,
    /// Explicit common overrides masked by a provider override.
    pub shadowed_keys: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    EmptyPatch,
    InvalidNumber(&'static str),
    InvalidToml,
    AutoCompactExceedsContext,
}

fn parse(raw: &str) -> Result<DocumentMut, PlanError> {
    raw.parse().map_err(|_| PlanError::InvalidToml)
}

fn checked(key: &'static str, value: u64) -> Result<i64, PlanError> {
    i64::try_from(value)
        .ok()
        .filter(|n| *n > 0)
        .ok_or(PlanError::InvalidNumber(key))
}

fn read(document: &DocumentMut, key: &'static str) -> Result<Option<i64>, PlanError> {
    document
        .get(key)
        .map(|item| {
            item.as_integer()
                .filter(|n| *n > 0)
                .ok_or(PlanError::InvalidNumber(key))
        })
        .transpose()
}

fn set(document: &mut DocumentMut, key: &'static str, number: i64) {
    if let Some(Item::Value(old)) = document.get_mut(key) {
        if old.as_integer() == Some(number) {
            return;
        }
        // Preserve inline comments and key spacing when updating a scalar.
        let decor = old.decor().clone();
        let mut next = Value::from(number);
        *next.decor_mut() = decor;
        *old = next;
    } else {
        document[key] = toml_edit::value(number);
    }
}

/// Plan master-data overrides using the desktop's existing provider-over-common
/// merger. No implicit cache default is represented as a user override.
pub fn plan_context_patch(
    common: &str,
    provider: &str,
    target: Target,
    patch: ContextPatch,
) -> Result<ContextPlan, PlanError> {
    if patch.model_context_window.is_none() && patch.model_auto_compact_token_limit.is_none() {
        return Err(PlanError::EmptyPatch);
    }
    // Validate the requested pair as well as the resulting effective pair.
    // A provider override must not hide an invalid common-data request.
    if let (Some(context), Some(limit)) = (
        patch.model_context_window,
        patch.model_auto_compact_token_limit,
    ) {
        if limit > context {
            return Err(PlanError::AutoCompactExceedsContext);
        }
    }
    let mut common_doc = parse(common)?;
    let mut provider_doc = parse(provider)?;
    let (document, original) = match target {
        Target::Common => (&mut common_doc, common),
        Target::Provider => (&mut provider_doc, provider),
    };
    for (key, value) in [
        (CONTEXT, patch.model_context_window),
        (COMPACT, patch.model_auto_compact_token_limit),
    ] {
        if let Some(value) = value {
            set(document, key, checked(key, value)?);
        }
    }
    let candidate = document.to_string();
    let changed = candidate != original;
    // Validate each master-data layer too: a provider must not mask invalid
    // common values that would reappear when that provider is changed.
    for layer in [&common_doc, &provider_doc] {
        if let (Some(context), Some(limit)) = (read(layer, CONTEXT)?, read(layer, COMPACT)?) {
            if limit > context {
                return Err(PlanError::AutoCompactExceedsContext);
            }
        }
    }
    // This is the sole shared merger, not an independent CLI projection path.
    let effective = parse(
        &append_toml_configs(&provider_doc.to_string(), &common_doc.to_string())
            .map_err(|_| PlanError::InvalidToml)?,
    )?;
    let explain = |key| -> Result<EffectiveOverride, PlanError> {
        let value = read(&effective, key)?;
        let source = if provider_doc.contains_key(key) {
            Source::Provider
        } else if common_doc.contains_key(key) {
            Source::Common
        } else {
            Source::Unspecified
        };
        Ok(EffectiveOverride { value, source })
    };
    let context = explain(CONTEXT)?;
    let auto_compact = explain(COMPACT)?;
    if let (Some(context), Some(limit)) = (context.value, auto_compact.value) {
        if limit > context {
            return Err(PlanError::AutoCompactExceedsContext);
        }
    }
    let shadowed_keys = [
        (CONTEXT, patch.model_context_window),
        (COMPACT, patch.model_auto_compact_token_limit),
    ]
    .into_iter()
    .filter_map(|(key, requested)| {
        (target == Target::Common && requested.is_some() && provider_doc.contains_key(key))
            .then_some(key)
    })
    .collect();
    Ok(ContextPlan {
        candidate,
        changed,
        context,
        auto_compact,
        shadowed_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn patch(context: Option<u64>, compact: Option<u64>) -> ContextPatch {
        ContextPatch {
            model_context_window: context,
            model_auto_compact_token_limit: compact,
        }
    }
    #[test]
    fn plans_requested_root_values_without_io_or_capacity_claim() {
        let plan = plan_context_patch(
            "",
            "[model_providers.custom]\nname='Synthetic'\n",
            Target::Common,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        assert_eq!(
            plan.context,
            EffectiveOverride {
                value: Some(1_000_000),
                source: Source::Common
            }
        );
        assert_eq!(plan.auto_compact.value, Some(900_000));
        let document = parse(&plan.candidate).unwrap();
        assert_eq!(document[CONTEXT].as_integer(), Some(1_000_000));
        assert!(!document.contains_key("model_providers"));
    }
    #[test]
    fn preserves_comments_unknown_sections_and_auth_text() {
        let raw = "# keep header\nmodel_context_window = 500000 # keep inline\nunknown = 'value'\n[auth]\nmode='synthetic'\n[mcp_servers.test]\ncommand='synthetic'\n[projects.'synthetic']\ntrust_level='trusted'\n";
        let plan =
            plan_context_patch(raw, "", Target::Common, patch(Some(1_000_000), None)).unwrap();
        assert_eq!(plan.candidate, raw.replace("500000", "1000000"));
    }
    #[test]
    fn repeated_patch_is_byte_noop() {
        let first = plan_context_patch(
            "",
            "",
            Target::Common,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        let second = plan_context_patch(
            &first.candidate,
            "",
            Target::Common,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        assert!(!second.changed);
        assert_eq!(first.candidate, second.candidate);
    }
    #[test]
    fn equal_numeric_patch_preserves_original_lexical_spelling() {
        let raw = "model_context_window = 1_000_000 # preserve\n";
        let plan =
            plan_context_patch(raw, "", Target::Common, patch(Some(1_000_000), None)).unwrap();
        assert!(!plan.changed);
        assert_eq!(plan.candidate, raw);
    }
    #[test]
    fn provider_wins_and_shadowing_is_explicit() {
        let plan = plan_context_patch(
            "",
            "model_context_window=500000\n",
            Target::Common,
            patch(Some(1_000_000), None),
        )
        .unwrap();
        assert_eq!(plan.context.value, Some(500_000));
        assert_eq!(plan.context.source, Source::Provider);
        assert_eq!(plan.shadowed_keys, vec![CONTEXT]);
    }
    #[test]
    fn validates_effective_threshold_even_when_only_context_changes() {
        assert_eq!(
            plan_context_patch(
                "model_auto_compact_token_limit=900000\n",
                "",
                Target::Common,
                patch(Some(500_000), None)
            ),
            Err(PlanError::AutoCompactExceedsContext)
        );
        assert_eq!(
            plan_context_patch(
                "",
                "model_context_window=500000\n",
                Target::Common,
                patch(None, Some(900_000))
            ),
            Err(PlanError::AutoCompactExceedsContext)
        );
    }
    #[test]
    fn provider_cannot_hide_invalid_common_master_data() {
        assert_eq!(
            plan_context_patch(
                "model_auto_compact_token_limit=900000\n",
                "model_context_window=2000000\n",
                Target::Common,
                patch(Some(500_000), None)
            ),
            Err(PlanError::AutoCompactExceedsContext)
        );
        assert_eq!(
            plan_context_patch(
                "model_auto_compact_token_limit='bad'\n",
                "model_auto_compact_token_limit=1\n",
                Target::Common,
                patch(Some(10), None)
            ),
            Err(PlanError::InvalidNumber(COMPACT))
        );
        assert_eq!(
            plan_context_patch(
                "",
                "model_context_window=2000000\n",
                Target::Common,
                patch(Some(500_000), Some(900_000))
            ),
            Err(PlanError::AutoCompactExceedsContext)
        );
    }
    #[test]
    fn absent_context_is_not_invented_from_model_cache() {
        let plan = plan_context_patch("", "", Target::Common, patch(None, Some(900_000))).unwrap();
        assert_eq!(
            plan.context,
            EffectiveOverride {
                value: None,
                source: Source::Unspecified
            }
        );
    }
    #[test]
    fn rejects_zero_overflow_empty_and_invalid_toml() {
        for value in [0, i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(
                plan_context_patch("", "", Target::Common, patch(Some(value), None)),
                Err(PlanError::InvalidNumber(CONTEXT))
            );
            assert_eq!(
                plan_context_patch("", "", Target::Common, patch(None, Some(value))),
                Err(PlanError::InvalidNumber(COMPACT))
            );
        }
        assert_eq!(
            plan_context_patch("", "", Target::Common, patch(None, None)),
            Err(PlanError::EmptyPatch)
        );
        assert_eq!(
            plan_context_patch("[broken", "", Target::Common, patch(Some(1), None)),
            Err(PlanError::InvalidToml)
        );
    }
    #[test]
    fn rejects_non_integer_effective_value_and_allows_equal_limit() {
        assert_eq!(
            plan_context_patch(
                "model_auto_compact_token_limit='bad'",
                "",
                Target::Common,
                patch(Some(10), None)
            ),
            Err(PlanError::InvalidNumber(COMPACT))
        );
        assert!(plan_context_patch("", "", Target::Provider, patch(Some(10), Some(10))).is_ok());
    }
}
