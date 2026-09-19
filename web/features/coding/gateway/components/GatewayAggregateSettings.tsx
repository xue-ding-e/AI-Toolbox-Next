import React from 'react';
import { Switch } from 'antd';
import { ArrowDown, ArrowUp, GripVertical, Loader2, Route } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import { CSS } from '@dnd-kit/utilities';
import {
  DEFAULT_AGGREGATE_SEPARATOR,
  engageProxyGatewayAggregate,
  getProxyGatewayAggregateDraft,
  getProxyGatewayCliStatuses,
  restoreProxyGatewayCliDirect,
  saveProxyGatewayAggregateDraft,
  type GatewayAggregateConfig,
  type GatewayCliKey,
  type GatewayAggregateNamingMode,
  type GatewayCliTakeoverStatus,
} from '@/services';
import { refreshTrayMenu } from '@/services/appApi';
import { listCodexProviders } from '@/services/codexApi';
import type { CodexProvider } from '@/types/codex';
import { isCodexLocalProviderId } from '@/features/coding/codex/utils/localProvider';
import { primaryCodexProviderNeedsGatewayProxy } from '@/features/coding/codex/utils/codexGatewayProxyNeed';
import {
  aliasesForSelectedSites,
  buildGatewayAggregateSitePreviewSlug,
  getGatewayProviderProfilesVersion,
  getGatewayAggregateConfigVersion,
  notifyGatewayAggregateConfigChanged,
  isAggregateSiteId,
  moveAggregateSite,
  normalizeGatewayAggregateAliases,
  normalizeGatewayAggregateSiteIds,
  resolveAggregateFormSeed,
  restoreDirectUnavailableHintKey,
  subscribeGatewayProviderProfiles,
  subscribeGatewayAggregateConfig,
  runGatewayAggregateMutation,
  toAggregateSiteCandidates,
  validateGatewayAggregateSeparator,
  validateGatewayAggregateAlias,
  type GatewayAggregateSiteCandidate,
} from '@/features/coding/shared/gateway';
import styles from './GatewayAggregateSettings.module.less';

// The backend aggregate manifest and model catalog are Codex-specific for now.
// Keep the selector honest instead of exposing CLI choices that the command
// layer will reject.
type AggregateCliKey = Extract<GatewayCliKey, 'codex'>;

const AGGREGATE_CLI_KEYS: AggregateCliKey[] = [
  'codex',
];

/**
 * Reuse the providers page data source per CLI; the gateway cannot reach any
 * provider the CLI page does not already own, so no extra request is invented.
 *
 * Returns the raw records too: the settings block has to re-check the primary
 * provider's protocol requirement before restoring direct mode, and that needs
 * the provider's `meta`/`settingsConfig`, not just its id and name.
 */
const loadProviders = async (
  cliKey: AggregateCliKey,
): Promise<{ providers: CodexProvider[]; candidates: GatewayAggregateSiteCandidate[] }> => {
  switch (cliKey) {
    case 'codex': {
      const providers = await listCodexProviders();
      return { providers, candidates: toAggregateSiteCandidates(providers) };
    }
    default:
      return { providers: [], candidates: [] };
  }
};

const formatError = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

interface SortableSiteRowProps {
  candidate: GatewayAggregateSiteCandidate;
  index: number;
  lastIndex: number;
  /** Aggregate mode cannot be empty, so the only selected site stays selected. */
  canDeselect: boolean;
  routePreview: string;
  onToggleSite: (siteId: string, checked: boolean) => void;
  onMoveSite: (siteId: string, direction: 'up' | 'down') => void;
  alias: string;
  onAliasChange: (siteId: string, alias: string) => void;
  onAliasCommit: () => void;
}

/**
 * Selected site row. Order is the aggregate fallback priority, so it is
 * reorderable by drag handle and by keyboard-accessible up/down buttons
 * (DESIGN.md requires an equivalent non-drag path).
 */
const SortableSiteRow: React.FC<SortableSiteRowProps> = ({
  candidate,
  index,
  lastIndex,
  canDeselect,
  routePreview,
  onToggleSite,
  onMoveSite,
  alias,
  onAliasChange,
  onAliasCommit,
}) => {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: candidate.id,
  });
  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.6 : undefined,
  };

  return (
    <li ref={setNodeRef} style={style} className={styles.siteItem}>
      <span
        className={styles.dragHandle}
        title={t('gateway.aggregate.reorderHint')}
        aria-label={t('gateway.aggregate.reorderHint')}
        {...attributes}
        {...listeners}
      >
        <GripVertical size={13} aria-hidden="true" />
      </span>
      <input
        type="checkbox"
        checked
        disabled={!canDeselect}
        aria-label={candidate.name}
        title={canDeselect ? undefined : t('gateway.aggregate.lastSiteRequired')}
        onChange={(event) => onToggleSite(candidate.id, event.currentTarget.checked)}
      />
      <span className={styles.siteName} title={candidate.name}>
        {candidate.name}
        <span className={styles.siteRoute}>
          {t('gateway.aggregate.routePreview')}: <code>{routePreview}</code>
        </span>
      </span>
      <code className={styles.siteSlug} title={candidate.id}>
        {candidate.id}
      </code>
      <input
        className={styles.aliasInput}
        value={alias}
        maxLength={32}
        placeholder={t('gateway.aggregate.aliasPlaceholder')}
        aria-label={`${candidate.name}: ${t('gateway.aggregate.alias')}`}
        aria-invalid={alias.length > 0 && !validateGatewayAggregateAlias(alias)}
        onChange={(event) => onAliasChange(candidate.id, event.currentTarget.value)}
        onBlur={onAliasCommit}
      />
      <span className={styles.siteActions}>
        <button
          type="button"
          className={styles.iconButton}
          disabled={index === 0}
          aria-label={`${candidate.name}: ${t('gateway.aggregate.moveUp')}`}
          onClick={() => onMoveSite(candidate.id, 'up')}
        >
          <ArrowUp size={13} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={styles.iconButton}
          disabled={index === lastIndex}
          aria-label={`${candidate.name}: ${t('gateway.aggregate.moveDown')}`}
          onClick={() => onMoveSite(candidate.id, 'down')}
        >
          <ArrowDown size={13} aria-hidden="true" />
        </button>
      </span>
    </li>
  );
};

interface GatewayAggregateSettingsProps {
  /** Gateway must be running before any engage command can succeed. */
  running: boolean;
  /**
   * Notified after a successful engage/disengage so the settings panel can
   * refresh its own takeover list. Must be stable (useCallback with no deps) —
   * it is not called on load, so it cannot feed back into the seed effect.
   */
  onTakeoverChange?: () => void;
}

const GatewayAggregateSettings: React.FC<GatewayAggregateSettingsProps> = ({
  running,
  onTakeoverChange,
}) => {
  const { t } = useTranslation();
  const [cliKey, setCliKey] = React.useState<AggregateCliKey>('codex');
  const [candidates, setCandidates] = React.useState<GatewayAggregateSiteCandidate[]>([]);
  const [providers, setProviders] = React.useState<CodexProvider[]>([]);
  const [loadingSites, setLoadingSites] = React.useState(true);
  const [siteIds, setSiteIds] = React.useState<string[]>([]);
  const [separator, setSeparator] = React.useState<string>(DEFAULT_AGGREGATE_SEPARATOR);
  const [aliases, setAliases] = React.useState<Record<string, string>>({});
  const [naming, setNaming] = React.useState<GatewayAggregateNamingMode>('site_model');
  const [cliStatuses, setCliStatuses] = React.useState<GatewayCliTakeoverStatus[]>([]);
  const [busy, setBusy] = React.useState(false);
  // Bumped when the persisted draft has been (re)loaded, so the seed effect can
  // pick up a fresh draft without re-seeding on every unrelated status refresh —
  // re-seeding mid-edit would revert what the user just changed.
  const [draftLoadRevision, setDraftLoadRevision] = React.useState(0);
  const [droppedDraftSites, setDroppedDraftSites] = React.useState(false);
  const [notice, setNotice] = React.useState<{ kind: 'error' | 'success'; text: string } | null>(
    null,
  );
  const aggregateConfigVersion = React.useSyncExternalStore(
    subscribeGatewayAggregateConfig,
    getGatewayAggregateConfigVersion,
    getGatewayAggregateConfigVersion,
  );
  const revisionRef = React.useRef(0);
  const statusRequestRef = React.useRef(0);
  const mutationRevisionRef = React.useRef(0);
  const draftRequestRef = React.useRef(0);
  const savedDraftRef = React.useRef<GatewayAggregateConfig | null>(null);
  const mountedRef = React.useRef(true);
  const selectedStatus = React.useMemo(
    () => cliStatuses.find((status) => status.cli_key === cliKey) ?? null,
    [cliKey, cliStatuses],
  );
  const engaged = selectedStatus?.mode === 'aggregate';
  // The primary provider's protocol is partly read from the gateway provider
  // profile store (`getGatewayProviderApiFormatFromMeta`), which changes without
  // the provider list identity changing. Subscribe the same way the Codex page
  // and provider card do, otherwise editing that profile while this panel is open
  // leaves the guard below answering about the previous protocol.
  const gatewayProviderProfilesVersion = React.useSyncExternalStore(
    subscribeGatewayProviderProfiles,
    getGatewayProviderProfilesVersion,
    getGatewayProviderProfilesVersion,
  );
  // Aggregate names the first selected site as `primary_provider_id`. The shared
  // gateway dialog refuses to restore direct while that provider still needs the
  // gateway for protocol conversion, so this entry point must refuse too —
  // otherwise the switch silently writes a direct config Codex cannot use.
  const primaryNeedsProxy = React.useMemo(
    () =>
      primaryCodexProviderNeedsGatewayProxy(
        providers,
        selectedStatus?.primary_provider_id,
        isCodexLocalProviderId,
      ),
    [gatewayProviderProfilesVersion, providers, selectedStatus?.primary_provider_id],
  );
  const restoreDirectBlocked = engaged && primaryNeedsProxy.needsProxy;
  const restoreDirectBlockedHint = t(
    restoreDirectUnavailableHintKey(primaryNeedsProxy.reason),
    { cli: t(`settings.gateway.cli.${cliKey}`) },
  );
  const separatorError = validateGatewayAggregateSeparator(separator);
  // Unselected candidates keep answering to their provider id, so aliases are
  // validated against the full candidate set — the same set the backend checks.
  const candidateSiteIds = React.useMemo(
    () => candidates.map((candidate) => candidate.id),
    [candidates],
  );
  const normalizedSiteIds = normalizeGatewayAggregateSiteIds(siteIds);
  const staleSiteIds = React.useMemo(() => {
    const addressable = new Set(candidateSiteIds);
    return siteIds.filter((siteId) => !addressable.has(siteId));
  }, [candidateSiteIds, siteIds]);
  const staleAliasSiteIds = React.useMemo(() => {
    const addressable = new Set(candidateSiteIds);
    return Object.keys(aliases).filter((siteId) => !addressable.has(siteId));
  }, [aliases, candidateSiteIds]);
  const hasStaleConfig = staleSiteIds.length > 0 || staleAliasSiteIds.length > 0;
  const normalizedAliases = normalizeGatewayAggregateAliases(aliases, siteIds, candidateSiteIds);
  const canEngage =
    running &&
    normalizedSiteIds.length > 0 &&
    !hasStaleConfig &&
    separatorError === null &&
    normalizedAliases !== null &&
    !busy;
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const applyCliStatuses = React.useCallback(
    (statuses: GatewayCliTakeoverStatus[]) => {
      if (mountedRef.current) {
        setCliStatuses(statuses);
      }
    },
    [],
  );

  const refreshCliStatuses = React.useCallback(async () => {
    const request = statusRequestRef.current + 1;
    statusRequestRef.current = request;
    const statuses = await getProxyGatewayCliStatuses();
    if (mountedRef.current && statusRequestRef.current === request) {
      applyCliStatuses(statuses);
    }
    return statuses;
  }, [applyCliStatuses]);

  React.useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      statusRequestRef.current += 1;
      mutationRevisionRef.current += 1;
    };
  }, []);

  // Seed the takeover state from the backend manifest so reopening the settings
  // page shows what is actually routing, not an empty form.
  React.useEffect(() => {
    void refreshCliStatuses().catch(() => {
      if (mountedRef.current) {
        applyCliStatuses([]);
      }
    });
  }, [applyCliStatuses, refreshCliStatuses]);

  // Another editor (for example provider-save re-engagement) may rewrite the
  // aggregate manifest while this drawer is mounted. Re-read canonical status
  // so this draft cannot overwrite a newer configuration.
  React.useEffect(() => {
    if (aggregateConfigVersion === 0) {
      return;
    }
    void refreshCliStatuses().catch(() => undefined);
  }, [aggregateConfigVersion, refreshCliStatuses]);
  React.useEffect(() => {
    let disposed = false;
    const request = revisionRef.current + 1;
    revisionRef.current = request;
    setLoadingSites(true);

    const load = async () => {
      try {
        // The draft is a convenience file: a failed read must not block the site
        // list, it only means the form falls back to the applied provider.
        const [next, draft] = await Promise.all([
          loadProviders(cliKey),
          getProxyGatewayAggregateDraft(cliKey).catch(() => null),
        ]);
        if (disposed || revisionRef.current !== request) {
          return;
        }
        setProviders(next.providers);
        setCandidates(next.candidates);
        savedDraftRef.current = draft;
        setDraftLoadRevision((revision) => revision + 1);
      } catch (error) {
        if (!disposed && revisionRef.current === request) {
          setProviders([]);
          setCandidates([]);
          savedDraftRef.current = null;
          setNotice({ kind: 'error', text: formatError(error) });
        }
      } finally {
        if (!disposed && revisionRef.current === request) {
          setLoadingSites(false);
        }
      }
    };

    void load();
    return () => {
      disposed = true;
    };
  }, [cliKey]);

  // Seed the form for the selected CLI. An engaged manifest wins because it is
  // what is actually routing (and keeps unavailable sites visible as an invalid
  // config instead of silently rewriting a running takeover); otherwise the
  // persisted draft is used, and an empty draft falls back to the CLI's
  // currently applied provider. Aggregate mode cannot represent "no site", so the
  // form is never seeded empty while the CLI has an eligible site.
  //
  // The seed deliberately keys off the manifest's *content* (`activeAggregateKey`)
  // rather than the status object identity: statuses are re-fetched on unrelated
  // refreshes, and re-seeding on those would revert edits the user just made.
  const activeAggregateKey = React.useMemo(
    () =>
      JSON.stringify(
        selectedStatus?.mode === 'aggregate' ? selectedStatus.aggregate ?? null : null,
      ),
    [selectedStatus],
  );
  React.useEffect(() => {
    const seed = resolveAggregateFormSeed({
      activeConfig:
        selectedStatus?.mode === 'aggregate' ? selectedStatus.aggregate ?? null : null,
      draftConfig: savedDraftRef.current,
      candidates,
      appliedProviderId: providers.find((provider) => provider.isApplied)?.id ?? null,
    });
    setSeparator(seed.separator);
    setNaming(seed.naming);
    setAliases(seed.aliases);
    setSiteIds(seed.siteIds);
    setDroppedDraftSites(seed.droppedDraftSites);
    // `selectedStatus` is intentionally absent: its identity changes on every
    // status refresh, while `activeAggregateKey` only changes when the engaged
    // configuration actually changed. Re-seeding on a refresh would revert the
    // user's in-progress edits.
  }, [activeAggregateKey, candidates, draftLoadRevision, providers]);

  const runGatewayOperation = React.useCallback(
    async <T,>(
      execute: () => Promise<T>,
      successText: string,
      failureKey: 'enableFailed' | 'disableFailed',
    ) => {
      const request = mutationRevisionRef.current + 1;
      mutationRevisionRef.current = request;
      setBusy(true);
      setNotice(null);

      let succeeded = false;
      try {
        await runGatewayAggregateMutation(async () => {
          try {
            const result = await execute();
            // Refresh inside the lane so the next queued mutation cannot race
            // this operation's canonical status read.
            await refreshCliStatuses().catch(() => undefined);
            succeeded = true;
            return result;
          } catch (error) {
            // Failed commands can still leave a partial restore or stale local
            // draft. Always re-read canonical status before the next mutation.
            await refreshCliStatuses().catch(() => undefined);
            throw error;
          }
        });
        notifyGatewayAggregateConfigChanged();
        void refreshTrayMenu().catch(() => undefined);
      } catch (error) {
        if (mountedRef.current && mutationRevisionRef.current === request) {
          setNotice({
            kind: 'error',
            text: t(`gateway.aggregate.notice.${failureKey}`, { error: formatError(error) }),
          });
        }
      } finally {
        // The parent owns provider locks and must refresh on both success and
        // failure; the child only lets the newest request update its notice.
        onTakeoverChange?.();
        if (mountedRef.current && mutationRevisionRef.current === request) {
          if (succeeded) {
            setNotice({ kind: 'success', text: successText });
          }
          setBusy(false);
        }
      }
      return succeeded;
    },
    [onTakeoverChange, refreshCliStatuses, t],
  );

  const runEngage = React.useCallback(
    async (
      nextSiteIds: string[],
      nextSeparator: string,
      nextAliases: Record<string, string>,
      nextNaming: GatewayAggregateNamingMode,
    ) => {
      const succeeded = await runGatewayOperation(
        () =>
          engageProxyGatewayAggregate(
            cliKey,
            nextSiteIds,
            nextSeparator,
            nextAliases,
            nextNaming,
          ),
        t('gateway.aggregate.notice.enabled'),
        'enableFailed',
      );
      if (succeeded) {
        // The backend persists the accepted selection to the draft file too; keep
        // the in-memory copy in step so leaving aggregate mode re-seeds this form
        // with the last engaged selection instead of an older draft.
        savedDraftRef.current = {
          provider_ids: [...nextSiteIds],
          separator: nextSeparator,
          aliases: { ...nextAliases },
          naming: nextNaming,
        };
      }
      return succeeded;
    },
    [cliKey, runGatewayOperation, t],
  );

  /**
   * Persist the form as the aggregate draft, so the selection survives closing
   * this editor. Only used while the mode is not engaged: an engaged takeover
   * re-engages instead, because the manifest is what actually routes.
   */
  const persistAggregateDraft = React.useCallback(
    (next: {
      siteIds: string[];
      separator: string;
      aliases: Record<string, string>;
      naming: GatewayAggregateNamingMode;
    }) => {
      const request = draftRequestRef.current + 1;
      draftRequestRef.current = request;
      // Serialized through the aggregate mutation lane so a slow, older write
      // cannot land after a newer one and resurrect a stale selection.
      void runGatewayAggregateMutation(() =>
        saveProxyGatewayAggregateDraft(
          cliKey,
          next.siteIds,
          next.separator,
          aliasesForSelectedSites(next.aliases, next.siteIds),
          next.naming,
        ),
      )
        .then((saved) => {
          if (mountedRef.current && draftRequestRef.current === request) {
            savedDraftRef.current = saved;
            setDroppedDraftSites(false);
          }
        })
        .catch((error) => {
          if (mountedRef.current && draftRequestRef.current === request) {
            setNotice({
              kind: 'error',
              text: t('gateway.aggregate.notice.draftSaveFailed', { error: formatError(error) }),
            });
          }
        });
    },
    [cliKey, t],
  );

  /**
   * Apply a new site selection or order. Engaged: re-engage so the running
   * takeover follows immediately. Not engaged: save the draft, which is what
   * makes the selection survive leaving this editor.
   */
  const applySiteSelection = (nextSiteIds: string[]) => {
    setSiteIds(nextSiteIds);
    setDroppedDraftSites(false);
    // Aliases only address selected sites (the backend refuses anything else), so
    // deselecting a site drops its alias. Keeping it would leave the form
    // permanently invalid and disable the switch without any visible reason.
    setAliases((current) => aliasesForSelectedSites(current, nextSiteIds));
    if (!engaged) {
      persistAggregateDraft({ siteIds: nextSiteIds, separator, aliases, naming });
      return;
    }
    const nextAliases = normalizeGatewayAggregateAliases(aliases, nextSiteIds, candidateSiteIds);
    if (nextSiteIds.length > 0 && separatorError === null && nextAliases) {
      void runEngage(nextSiteIds, separator, nextAliases, naming);
    }
  };

  const handleToggle = async (checked: boolean) => {
    setNotice(null);
    if (!checked) {
      if (restoreDirectBlocked) {
        setNotice({ kind: 'error', text: restoreDirectBlockedHint });
        return;
      }
      await runGatewayOperation(
        () => restoreProxyGatewayCliDirect(cliKey),
        t('gateway.aggregate.notice.disabled'),
        'disableFailed',
      );
      return;
    }

    if (hasStaleConfig) {
      setNotice({ kind: 'error', text: t('gateway.aggregate.notice.invalidConfig') });
      return;
    }
    if (normalizedSiteIds.length === 0) {
      setNotice({ kind: 'error', text: t('gateway.aggregate.sitesRequired') });
      return;
    }
    if (validateGatewayAggregateSeparator(separator) !== null) {
      setNotice({ kind: 'error', text: t('gateway.aggregate.notice.invalidConfig') });
      return;
    }
    if (!normalizedAliases) {
      setNotice({ kind: 'error', text: t('gateway.aggregate.aliasInvalid') });
      return;
    }
    await runEngage(normalizedSiteIds, separator, normalizedAliases, naming);
  };

  const handleToggleSite = (siteId: string, checked: boolean) => {
    const nextSiteIds = checked
      ? normalizeGatewayAggregateSiteIds([...siteIds, siteId])
      : siteIds.filter((item) => item !== siteId);
    // Aggregate mode cannot represent an empty site list, so the last selected
    // site stays selected; leaving aggregate mode is the switch's job.
    if (nextSiteIds.length === 0) {
      setNotice({ kind: 'error', text: t('gateway.aggregate.lastSiteRequired') });
      return;
    }
    applySiteSelection(nextSiteIds);
  };

  const handleMoveSite = (siteId: string, direction: 'up' | 'down') => {
    applySiteSelection(moveAggregateSite(siteIds, siteId, direction));
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) {
      return;
    }
    const oldIndex = siteIds.indexOf(String(active.id));
    const newIndex = siteIds.indexOf(String(over.id));
    if (oldIndex < 0 || newIndex < 0) {
      return;
    }
    applySiteSelection(arrayMove(siteIds, oldIndex, newIndex));
  };

  const handleSeparatorCommit = () => {
    if (validateGatewayAggregateSeparator(separator) !== null) {
      return;
    }
    if (!engaged) {
      persistAggregateDraft({ siteIds, separator, aliases, naming });
      return;
    }
    if (siteIds.length > 0 && normalizedAliases) {
      void runEngage(siteIds, separator, normalizedAliases, naming);
    }
  };

  const handleAliasCommit = () => {
    const nextAliases = normalizeGatewayAggregateAliases(aliases, siteIds, candidateSiteIds);
    if (!nextAliases) {
      setNotice({ kind: 'error', text: t('gateway.aggregate.aliasInvalid') });
      return;
    }
    if (!engaged) {
      persistAggregateDraft({ siteIds, separator, aliases, naming });
      return;
    }
    if (siteIds.length === 0) {
      return;
    }
    if (separatorError === null) {
      void runEngage(siteIds, separator, nextAliases, naming);
    }
  };

  const handleSelectAllSites = () => {
    const nextSiteIds = candidates.map((candidate) => candidate.id);
    if (nextSiteIds.length === 0) {
      return;
    }
    applySiteSelection(nextSiteIds);
  };

  const selectedCandidates = siteIds
    .map((siteId) => candidates.find((candidate) => candidate.id === siteId))
    .filter((candidate): candidate is GatewayAggregateSiteCandidate => Boolean(candidate));
  const unselectedCandidates = candidates.filter((candidate) => !siteIds.includes(candidate.id));
  const separatorExample = buildGatewayAggregateSitePreviewSlug(
    candidates[0]?.id || 'site-id',
    separatorError === null ? separator : DEFAULT_AGGREGATE_SEPARATOR,
    naming,
    aliases,
  );
  const buildSiteRoutePreview = React.useCallback(
    (siteId: string) =>
      buildGatewayAggregateSitePreviewSlug(
        siteId,
        separatorError === null ? separator : DEFAULT_AGGREGATE_SEPARATOR,
        naming,
        aliases,
      ),
    [aliases, naming, separator, separatorError],
  );
  const invalidSiteIds = siteIds.filter((siteId) => !isAggregateSiteId(siteId));

  return (
    <div className={styles.settings}>
      <div className={styles.toolbar}>
        <label className={styles.cliPicker}>
          <span>{t('gateway.aggregate.cliLabel')}</span>
          <select
            className={styles.select}
            value={cliKey}
            onChange={(event) => setCliKey(event.currentTarget.value as AggregateCliKey)}
          >
            {AGGREGATE_CLI_KEYS.map((option) => (
              <option key={option} value={option}>
                {t(`settings.gateway.cli.${option}`)}
              </option>
            ))}
          </select>
        </label>
        <div className={styles.toggle}>
          <span className={styles.state} role="status">
            {engaged ? t('gateway.aggregate.enabledLabel') : t('gateway.aggregate.disabledLabel')}
          </span>
          <Switch
            size="small"
            checked={engaged}
            disabled={
              busy ||
              (!engaged && !canEngage) ||
              (!engaged && !running) ||
              restoreDirectBlocked
            }
            loading={busy}
            title={restoreDirectBlocked ? restoreDirectBlockedHint : undefined}
            aria-label={
              engaged ? t('gateway.aggregate.disable') : t('gateway.aggregate.enable')
            }
            onChange={(checked) => {
              void handleToggle(checked);
            }}
          />
        </div>
      </div>

      <div className={styles.fieldRow}>
        <div className={styles.fieldMeta}>
          <span className={styles.fieldLabel}>{t('gateway.aggregate.naming')}</span>
          <span className={styles.fieldHelp}>{t('gateway.aggregate.namingHint')}</span>
        </div>
        <div className={styles.fieldControl}>
           <select
             className={styles.select}
             value={naming}
              disabled={busy}
              aria-label={t('gateway.aggregate.naming')}
             onChange={(event) => {
               const nextNaming = event.currentTarget.value as GatewayAggregateNamingMode;
               setNaming(nextNaming);
               if (!engaged) {
                 persistAggregateDraft({ siteIds, separator, aliases, naming: nextNaming });
               } else if (normalizedAliases && siteIds.length > 0) {
                 void runEngage(siteIds, separator, normalizedAliases, nextNaming);
               }
            }}
          >
            <option value="site_model">{t('gateway.aggregate.namingSiteModel')}</option>
            <option value="model_at_site">{t('gateway.aggregate.namingModelAtSite')}</option>
            <option value="model_only">{t('gateway.aggregate.namingModelOnly')}</option>
          </select>
        </div>
      </div>

       <p className={styles.helper}>{t('gateway.aggregate.modeHint')}</p>
      {droppedDraftSites ? (
        <p className={styles.helper}>{t('gateway.aggregate.draftSitesDropped')}</p>
      ) : null}
      {restoreDirectBlocked ? <p className={styles.helper}>{restoreDirectBlockedHint}</p> : null}
      {!running ? <p className={styles.helper}>{t('gateway.aggregate.takeoverHint')}</p> : null}

      <div className={styles.fieldRow}>
        <div className={styles.fieldMeta}>
          <span className={styles.fieldLabel}>{t('gateway.aggregate.separator')}</span>
           <span className={styles.fieldHelp}>
             {t('gateway.aggregate.separatorHint', { example: separatorExample })}
           </span>
        </div>
        <div className={styles.fieldControl}>
          <input
             className={styles.separatorInput}
             value={separator}
              disabled={busy}
            placeholder={t('gateway.aggregate.separatorPlaceholder')}
            aria-label={t('gateway.aggregate.separator')}
            aria-invalid={separatorError !== null}
            onChange={(event) => setSeparator(event.currentTarget.value)}
            onBlur={handleSeparatorCommit}
          />
        </div>
      </div>
      {separatorError ? (
        <div className={styles.error} role="alert">
          {separatorError === 'empty'
            ? t('gateway.aggregate.separatorInvalidEmpty')
            : t('gateway.aggregate.separatorInvalidReserved')}
        </div>
      ) : null}

      {loadingSites ? (
        <div className={styles.loading}>
          <Loader2 size={14} className={styles.spin} aria-hidden="true" />
        </div>
      ) : candidates.length === 0 ? (
        <div className={styles.emptyState}>
          <span>{t('gateway.aggregate.noSites')}</span>
          <p>{t('gateway.aggregate.noSitesHint', { cli: t(`settings.gateway.cli.${cliKey}`) })}</p>
        </div>
      ) : (
        <>
           <div className={styles.listHeader}>
             <span className={styles.listTitle}>
               <Route size={12} aria-hidden="true" />
               {t('gateway.aggregate.selectedCount', { count: siteIds.length })}
             </span>
             <span className={styles.listActions}>
                {selectedCandidates.length < candidates.length ? (
                  <button
                    type="button"
                    className={styles.textButton}
                    onClick={handleSelectAllSites}
                  >
                    {t('gateway.aggregate.selectAll')}
                  </button>
                ) : null}
              </span>
            </div>

            <>
                <DndContext
                 sensors={sensors}
                 collisionDetection={closestCenter}
                 modifiers={[restrictToVerticalAxis]}
                 onDragEnd={handleDragEnd}
               >
                 <SortableContext items={siteIds} strategy={verticalListSortingStrategy}>
                   <ul className={styles.siteList}>
                     {selectedCandidates.map((candidate, index) => (
                       <SortableSiteRow
                         key={candidate.id}
                         candidate={candidate}
                         index={index}
                         lastIndex={selectedCandidates.length - 1}
                         canDeselect={selectedCandidates.length > 1}
                         routePreview={buildSiteRoutePreview(candidate.id)}
                         onToggleSite={handleToggleSite}
                         onMoveSite={handleMoveSite}
                         alias={aliases[candidate.id] ?? ''}
                         onAliasChange={(siteId, alias) => {
                           const nextAliases = { ...aliases, [siteId]: alias };
                           if (!alias.trim()) delete nextAliases[siteId];
                           setAliases(nextAliases);
                         }}
                         onAliasCommit={handleAliasCommit}
                       />
                     ))}
                   </ul>
                 </SortableContext>
               </DndContext>

               {unselectedCandidates.length > 0 ? (
                 <ul className={styles.siteList}>
                   {unselectedCandidates.map((candidate) => (
                     <li key={candidate.id} className={styles.siteItem}>
                       <input
                         type="checkbox"
                         checked={false}
                         aria-label={candidate.name}
                         onChange={(event) =>
                           handleToggleSite(candidate.id, event.currentTarget.checked)
                         }
                       />
                       <span className={styles.siteName} title={candidate.name}>
                         {candidate.name}
                         <span className={styles.siteRoute}>
                           {t('gateway.aggregate.routePreview')}:{' '}
                           <code>{buildSiteRoutePreview(candidate.id)}</code>
                         </span>
                       </span>
                       <code className={styles.siteSlug} title={candidate.id}>
                         {candidate.id}
                       </code>
                     </li>
                   ))}
                 </ul>
                ) : null}
            </>
        </>
      )}

      {invalidSiteIds.length > 0 ? (
        <div className={styles.error} role="alert">
          {t('gateway.aggregate.notice.invalidConfig')}
        </div>
      ) : null}
      {notice ? (
        <div
          className={notice.kind === 'error' ? styles.error : styles.success}
          role="status"
        >
          {notice.text}
        </div>
      ) : null}
    </div>
  );
};

export default GatewayAggregateSettings;
