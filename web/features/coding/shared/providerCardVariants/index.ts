/**
 * The three provider-card styles, fixed.
 *
 * Every CLI's provider list uses one of these, so the layouts stop drifting
 * apart one page at a time. Pick by the *shape of the CLI's provider model*,
 * not by taste:
 *
 * | Style | Use when | Traits |
 * |---|---|---|
 * | `OpenCodeStyleCard` | the CLI owns a model catalog and has no single active provider | uniform `id • SDK • endpoint` line, icon header actions, default chosen per model row |
 * | `CodexStyleCard` | the CLI owns a model catalog and has one active provider | free-form meta line, text-link apply, collapsible model section |
 * | `ClaudeStyleCard` | the CLI has no model catalog (models live in the config it writes) | labelled binding line, text-link apply, no model section |
 *
 * The mapping is not one-to-one with "which CLI": a CLI that later gains or
 * loses a model catalog changes style, and that is the intended signal.
 */
export { default as CardShell } from './CardShell';
export { default as OpenCodeStyleCard } from './OpenCodeStyleCard';
export { default as CodexStyleCard } from './CodexStyleCard';
export { InlineConnectivityButton } from './CodexStyleCard';
export { default as ClaudeStyleCard } from './ClaudeStyleCard';
export type {
  ProviderCardVariantProps,
  ProviderCardModel,
  ProviderCardMetaEntry,
  ProviderCardActions,
  ProviderCardState,
  ProviderCardModels,
} from './types';
