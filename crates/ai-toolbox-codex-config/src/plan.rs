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
    /// `None` leaves the existing value untouched; deletion is not supported.
    pub model_context_window: Option<u64>,
    /// `None` leaves the existing value untouched; deletion is not supported.
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
pub struct ContextProjectionPlan {
    /// Master-data candidate and provider-over-common values. Unrequested live
    /// values may differ: this narrow plan does not reconcile unrelated drift.
    pub master: ContextPlan,
    /// Candidate live TOML only; no file has been written or verified. Both
    /// candidates may contain secrets and must not be used as default CLI output.
    pub live_candidate: String,
    pub live_changed: bool,
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

/// Plan the explicitly requested context keys on caller-supplied live TOML.
///
/// Only requested keys are projected, using the master plan's effective values,
/// including provider overrides that shadow a common patch. `None` never deletes
/// or reconciles a live key. Missing requested keys are inserted at the root.
/// Existing live context keys must already be positive integers; malformed
/// values are rejected, not implicitly repaired or replaced as whole tables.
/// The final live pair is validated without changing its unrequested member.
/// All other live items remain untouched; auth.json is not an input or output.
/// This does not perform the desktop service's full provider application.
pub fn plan_context_projection(
    live: &str,
    common: &str,
    provider: &str,
    target: Target,
    patch: ContextPatch,
) -> Result<ContextProjectionPlan, PlanError> {
    let master = plan_context_patch(common, provider, target, patch)?;
    let mut live_doc = parse(live)?;
    // Do not silently repair malformed live values or discard table children.
    // A pair of valid numbers may still be corrected by the requested patch.
    read(&live_doc, CONTEXT)?;
    read(&live_doc, COMPACT)?;
    for (key, requested, effective) in [
        (CONTEXT, patch.model_context_window, master.context.value),
        (
            COMPACT,
            patch.model_auto_compact_token_limit,
            master.auto_compact.value,
        ),
    ] {
        if requested.is_some() {
            // A requested key must have an effective override. Fail closed
            // instead of interpreting an absent value as a deletion.
            set(
                &mut live_doc,
                key,
                effective.ok_or(PlanError::InvalidNumber(key))?,
            );
        }
    }
    if let (Some(context), Some(limit)) = (read(&live_doc, CONTEXT)?, read(&live_doc, COMPACT)?) {
        if limit > context {
            return Err(PlanError::AutoCompactExceedsContext);
        }
    }
    let live_candidate = live_doc.to_string();
    let live_changed = live_candidate != live;
    Ok(ContextProjectionPlan {
        master,
        live_candidate,
        live_changed,
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

    #[test]
    fn projection_preserves_non_context_text_and_auth() {
        let live = "#:schema https://example.invalid/codex.schema.json\n# Keep header\nmodel_context_window = 500000 # keep context\nmodel_auto_compact_token_limit = 400000 # keep compact\nmodel = 'synthetic-model'\nmodel_provider = 'custom'\nmodel_catalog_json = 'synthetic-catalog.json'\nunknown = { retained = true }\n\n[model_providers.custom]\nname = 'Synthetic'\nrequires_openai_auth = true # keep auth mode\nexperimental_bearer_token = 'synthetic-token'\nenv_key = 'SYNTHETIC_KEY'\n\n[model_providers.other]\nname = 'Other'\nwire_api = 'responses'\n\n[auth]\nmode = 'synthetic'\n\n[mcp_servers.test]\ncommand = 'synthetic'\n\n[features]\nplugins = true # keep runtime feature\n\n[plugins.synthetic]\nenabled = true\n\n[projects.'synthetic-project']\ntrust_level = 'trusted'\n";
        let plan = plan_context_projection(
            live,
            "model = 'not-projected'\n",
            "[model_providers.custom]\nrequires_openai_auth = false\n",
            Target::Common,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        assert!(plan.master.changed);
        assert!(plan.live_changed);
        assert_eq!(
            plan.live_candidate,
            live.replace("= 500000", "= 1000000")
                .replace("= 400000", "= 900000")
        );
    }

    #[test]
    fn projection_uses_effective_provider_values_for_shadowed_common_patch() {
        let live = "model_context_window = 500_000 # keep spelling\nmodel_auto_compact_token_limit = 400_000\n";
        let provider = "model_context_window=500000\nmodel_auto_compact_token_limit=400000\n";
        let plan = plan_context_projection(
            live,
            "",
            provider,
            Target::Common,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        assert!(plan.master.changed);
        assert!(!plan.live_changed);
        assert_eq!(plan.live_candidate, live);
        assert_eq!(plan.master.context.source, Source::Provider);
        assert_eq!(plan.master.context.value, Some(500_000));
        assert_eq!(plan.master.auto_compact.source, Source::Provider);
        assert_eq!(plan.master.auto_compact.value, Some(400_000));
        assert_eq!(plan.master.shadowed_keys, vec![CONTEXT, COMPACT]);
        let changed = plan_context_projection(
            "model_context_window=1\nmodel_auto_compact_token_limit=1\n",
            "",
            provider,
            Target::Common,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        assert_eq!(
            changed.live_candidate,
            "model_context_window=500000\nmodel_auto_compact_token_limit=400000\n"
        );
    }

    #[test]
    fn projection_changes_only_requested_key_without_reconciling_live_drift() {
        let live = "model_context_window = 500_000\nmodel_auto_compact_token_limit = 450_000 # keep drift\n";
        let plan = plan_context_projection(
            live,
            "model_auto_compact_token_limit=400000\n",
            "",
            Target::Provider,
            patch(Some(1_000_000), None),
        )
        .unwrap();
        assert_eq!(plan.master.context.source, Source::Provider);
        assert_eq!(plan.master.auto_compact.value, Some(400_000));
        assert_eq!(
            plan.live_candidate,
            live.replace("= 500_000", "= 1000000")
        );
    }

    #[test]
    fn projection_replay_is_byte_noop_and_preserves_equal_number_spelling() {
        let common = "model_context_window = 1_000_000\nmodel_auto_compact_token_limit = 900_000\n";
        let first = plan_context_projection(
            "model_context_window = 1_000_000 # keep spelling\n",
            common,
            "",
            Target::Common,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        assert!(!first.master.changed);
        assert!(first.live_changed);
        assert!(first
            .live_candidate
            .contains("model_context_window = 1_000_000 # keep spelling"));
        let replay = plan_context_projection(
            &first.live_candidate,
            &first.master.candidate,
            "",
            Target::Common,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        assert!(!replay.master.changed);
        assert!(!replay.live_changed);
        assert_eq!(replay.live_candidate, first.live_candidate);
    }

    #[test]
    fn projection_inserts_requested_missing_keys_at_root() {
        let live = "# Keep table\n[model_providers.custom]\nname='Synthetic'\n";
        let plan = plan_context_projection(
            live,
            "",
            "",
            Target::Provider,
            patch(Some(1_000_000), Some(900_000)),
        )
        .unwrap();
        let document = parse(&plan.live_candidate).unwrap();
        assert_eq!(document[CONTEXT].as_integer(), Some(1_000_000));
        assert_eq!(document[COMPACT].as_integer(), Some(900_000));
        let provider = document["model_providers"]["custom"].as_table().unwrap();
        assert!(!provider.contains_key(CONTEXT));
        assert!(!provider.contains_key(COMPACT));
        assert!(plan.live_candidate.ends_with(live));
    }

    #[test]
    fn projection_never_deletes_or_invents_unrequested_keys() {
        for live in [
            "model_context_window = 1_000_000 # not in master data\n",
            "",
        ] {
            let plan = plan_context_projection(
                live,
                "",
                "",
                Target::Common,
                patch(None, Some(900_000)),
            )
            .unwrap();
            assert_eq!(plan.master.context.value, None);
            assert_eq!(plan.master.context.source, Source::Unspecified);
            assert_eq!(
                plan.live_candidate,
                format!("{live}model_auto_compact_token_limit = 900000\n")
            );
        }
        assert_eq!(
            plan_context_projection(
                "model_context_window=1\n",
                "",
                "",
                Target::Common,
                patch(None, None)
            ),
            Err(PlanError::EmptyPatch)
        );
    }

    #[test]
    fn projection_rejects_invalid_live_values_even_when_requested() {
        assert_eq!(
            plan_context_projection("[broken", "", "", Target::Common, patch(Some(10), None)),
            Err(PlanError::InvalidToml)
        );
        for invalid in ["'bad'", "0", "-1", "1.5", "true", "[]", "{}"] {
            for (live, requested, key) in [
                (
                    format!("model_auto_compact_token_limit={invalid}\n"),
                    patch(Some(10), None),
                    COMPACT,
                ),
                (
                    format!("model_auto_compact_token_limit={invalid}\n"),
                    patch(None, Some(10)),
                    COMPACT,
                ),
                (
                    format!("model_context_window={invalid}\n"),
                    patch(None, Some(10)),
                    CONTEXT,
                ),
                (
                    format!("model_context_window={invalid}\n"),
                    patch(Some(10), None),
                    CONTEXT,
                ),
            ] {
                assert_eq!(
                    plan_context_projection(&live, "", "", Target::Common, requested),
                    Err(PlanError::InvalidNumber(key))
                );
            }
        }
        for key in [CONTEXT, COMPACT] {
            let live = format!("[{key}]\nunknown='retained'\n");
            assert_eq!(
                plan_context_projection(
                    &live,
                    "",
                    "",
                    Target::Common,
                    patch(Some(10), Some(9))
                ),
                Err(PlanError::InvalidNumber(key))
            );
        }
    }

    #[test]
    fn projection_rejects_invalid_live_pair_without_touching_inputs() {
        for (live, common, provider, requested) in [
            (
                "model_context_window=1000000\nmodel_auto_compact_token_limit=900000\n",
                "model_auto_compact_token_limit=400000\n",
                "",
                patch(Some(500_000), None),
            ),
            (
                "model_context_window=500000\n",
                "model_context_window=1000000\n",
                "",
                patch(None, Some(900_000)),
            ),
            (
                "model_context_window=1000000\nmodel_auto_compact_token_limit=900000\n",
                "",
                "model_context_window=500000\n",
                patch(Some(2_000_000), None),
            ),
        ] {
            let inputs = (live.to_string(), common.to_string(), provider.to_string());
            let original = inputs.clone();
            assert_eq!(
                plan_context_projection(&inputs.0, &inputs.1, &inputs.2, Target::Common, requested),
                Err(PlanError::AutoCompactExceedsContext)
            );
            assert_eq!(inputs, original);
        }
        let corrected = plan_context_projection(
            "model_context_window=10\nmodel_auto_compact_token_limit=20\n",
            "",
            "",
            Target::Common,
            patch(Some(30), None),
        )
        .unwrap();
        assert_eq!(
            corrected.live_candidate,
            "model_context_window=30\nmodel_auto_compact_token_limit=20\n"
        );
    }

    #[test]
    fn projection_rejects_invalid_requests_and_master_data_all_or_nothing() {
        let live = "model_context_window=10\nmodel_auto_compact_token_limit=9\n";
        for number in [0, i64::MAX as u64 + 1, u64::MAX] {
            for (requested, key) in [
                (patch(Some(number), None), CONTEXT),
                (patch(None, Some(number)), COMPACT),
            ] {
                assert_eq!(
                    plan_context_projection(live, "", "", Target::Common, requested),
                    Err(PlanError::InvalidNumber(key))
                );
            }
        }
        for (common, provider, requested, expected) in [
            ("[broken", "", patch(Some(10), None), PlanError::InvalidToml),
            ("", "[broken", patch(Some(10), None), PlanError::InvalidToml),
            // A valid first field must not produce a partial projection.
            (
                "",
                "",
                patch(Some(10), Some(0)),
                PlanError::InvalidNumber(COMPACT),
            ),
            (
                "model_auto_compact_token_limit='bad'\n",
                "model_auto_compact_token_limit=1\n",
                patch(Some(10), None),
                PlanError::InvalidNumber(COMPACT),
            ),
            (
                "",
                "model_context_window=5\n",
                patch(Some(10), Some(9)),
                PlanError::AutoCompactExceedsContext,
            ),
            (
                "",
                "model_context_window=100\n",
                patch(Some(5), Some(9)),
                PlanError::AutoCompactExceedsContext,
            ),
        ] {
            assert_eq!(
                plan_context_projection(live, common, provider, Target::Common, requested),
                Err(expected)
            );
        }
    }

    #[test]
    fn projection_accepts_positive_i64_boundaries_and_equal_threshold() {
        for number in [1, i64::MAX as u64] {
            let plan = plan_context_projection(
                "",
                "",
                "",
                Target::Provider,
                patch(Some(number), Some(number)),
            )
            .unwrap();
            let document = parse(&plan.live_candidate).unwrap();
            assert_eq!(document[CONTEXT].as_integer(), Some(number as i64));
            assert_eq!(document[COMPACT].as_integer(), Some(number as i64));
        }
    }
}
