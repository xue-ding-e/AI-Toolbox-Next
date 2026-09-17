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
  getProxyGatewayCliStatuses,
  restoreProxyGatewayCliDirect,
  type GatewayCliKey,
  type GatewayAggregateNamingMode,
  type GatewayCliTakeoverStatus,
} from '@/services';
import { listCodexProviders } from '@/services/codexApi';
import type { CodexProvider } from '@/types/codex';
import { isCodexLocalProviderId } from '@/features/coding/codex/utils/localProvider';
import { primaryCodexProviderNeedsGatewayProxy } from '@/features/coding/codex/utils/codexGatewayProxyNeed';
import {
  buildGatewayAggregateModelSlug,
  isAggregateSiteId,
  moveAggregateSite,
  normalizeGatewayAggregateAliases,
  normalizeGatewayAggregateSiteIds,
  reconcileAggregateSiteSelection,
  restoreDirectUnavailableHintKey,
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
        aria-label={candidate.name}
        onChange={(event) => onToggleSite(candidate.id, event.currentTarget.checked)}
      />
      <span className={styles.siteName} title={candidate.name}>
        {candidate.name}
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
  const [notice, setNotice] = React.useState<{ kind: 'error' | 'success'; text: string } | null>(
    null,
  );
  const revisionRef = React.useRef(0);

  const selectedStatus = React.useMemo(
    () => cliStatuses.find((status) => status.cli_key === cliKey) ?? null,
    [cliKey, cliStatuses],
  );
  const engaged = selectedStatus?.mode === 'aggregate';
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
    [providers, selectedStatus?.primary_provider_id],
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
  const normalizedAliases = normalizeGatewayAggregateAliases(aliases, siteIds, candidateSiteIds);
  const canEngage =
    running && siteIds.length > 0 && separatorError === null && normalizedAliases !== null && !busy;
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const applyCliStatuses = React.useCallback(
    (statuses: GatewayCliTakeoverStatus[]) => {
      setCliStatuses(statuses);
    },
    [],
  );

  const refreshCliStatuses = React.useCallback(async () => {
    const statuses = await getProxyGatewayCliStatuses();
    applyCliStatuses(statuses);
    return statuses;
  }, [applyCliStatuses]);

  // Seed the takeover state from the backend manifest so reopening the settings
  // page shows what is actually routing, not an empty form.
  React.useEffect(() => {
    let disposed = false;
    const load = async () => {
      try {
        const statuses = await getProxyGatewayCliStatuses();
        if (!disposed) {
          applyCliStatuses(statuses);
        }
      } catch {
        if (!disposed) {
          applyCliStatuses([]);
        }
      }
    };
    void load();
    return () => {
      disposed = true;
    };
  }, [applyCliStatuses]);

  React.useEffect(() => {
    let disposed = false;
    const request = revisionRef.current + 1;
    revisionRef.current = request;
    setLoadingSites(true);

    const load = async () => {
      try {
        const next = await loadProviders(cliKey);
        if (disposed || revisionRef.current !== request) {
          return;
        }
        setProviders(next.providers);
        setCandidates(next.candidates);
      } catch (error) {
        if (!disposed && revisionRef.current === request) {
          setProviders([]);
          setCandidates([]);
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

  // Load the saved aggregate config for the selected CLI, and drop sites that
  // are no longer proxyable instead of showing them as still selected. Keyed on
  // the backend status so a re-engage round trip re-seeds the canonical list.
  React.useEffect(() => {
    const saved = selectedStatus?.mode === 'aggregate' ? selectedStatus.aggregate ?? null : null;
    setSeparator(
      saved?.separator && validateGatewayAggregateSeparator(saved.separator) === null
        ? saved.separator
        : DEFAULT_AGGREGATE_SEPARATOR,
    );
    setNaming(saved?.naming ?? 'site_model');
    if (!saved) {
      setAliases({});
      setSiteIds([]);
      return;
    }
    const { siteIds: nextSiteIds } = reconcileAggregateSiteSelection(
      saved.provider_ids,
      candidates,
    );
    const selectedIds = new Set(nextSiteIds);
    // An alias keyed on a site that is no longer selectable (deleted or newly
    // disabled provider) can never be edited — its row is not rendered — and
    // `normalizeGatewayAggregateAliases` rejects the whole map for it, which
    // would leave the form permanently un-engageable. Drop those keys instead.
    setAliases(
      Object.fromEntries(
        Object.entries(saved.aliases ?? {}).filter(([siteId]) => selectedIds.has(siteId)),
      ),
    );
    setSiteIds(nextSiteIds);
  }, [candidates, selectedStatus]);

  const runEngage = React.useCallback(
    async (
      nextSiteIds: string[],
      nextSeparator: string,
      nextAliases: Record<string, string>,
      nextNaming: GatewayAggregateNamingMode,
    ) => {
      setBusy(true);
      setNotice(null);
      try {
        await engageProxyGatewayAggregate(cliKey, nextSiteIds, nextSeparator, nextAliases, nextNaming);
        await refreshCliStatuses();
        onTakeoverChange?.();
        setNotice({ kind: 'success', text: t('gateway.aggregate.notice.enabled') });
      } catch (error) {
        setNotice({
          kind: 'error',
          text: t('gateway.aggregate.notice.enableFailed', { error: formatError(error) }),
        });
      } finally {
        setBusy(false);
      }
    },
    [cliKey, onTakeoverChange, refreshCliStatuses, t],
  );

  const handleToggle = async (checked: boolean) => {
    setNotice(null);
    if (!checked) {
      if (restoreDirectBlocked) {
        setNotice({ kind: 'error', text: restoreDirectBlockedHint });
        return;
      }
      setBusy(true);
      try {
        await restoreProxyGatewayCliDirect(cliKey);
        await refreshCliStatuses();
        onTakeoverChange?.();
        setNotice({ kind: 'success', text: t('gateway.aggregate.notice.disabled') });
      } catch (error) {
        setNotice({
          kind: 'error',
          text: t('gateway.aggregate.notice.disableFailed', { error: formatError(error) }),
        });
      } finally {
        setBusy(false);
      }
      return;
    }

    const normalizedSiteIds = normalizeGatewayAggregateSiteIds(siteIds);
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

  const handleToggleSite = async (siteId: string, checked: boolean) => {
    const nextSiteIds = checked
      ? normalizeGatewayAggregateSiteIds([...siteIds, siteId])
      : siteIds.filter((item) => item !== siteId);
    setSiteIds(nextSiteIds);
    // Auto-save: a running aggregate takeover must follow the new site list.
    if (!engaged) {
      return;
    }
    if (nextSiteIds.length === 0) {
      // Aggregate mode cannot represent an empty site list. Restoring direct
      // mode is safer than leaving the backend on the stale selection.
      await handleToggle(false);
      return;
    }
    const nextAliases = normalizeGatewayAggregateAliases(aliases, nextSiteIds, candidateSiteIds);
    if (separatorError === null && nextAliases) {
      void runEngage(nextSiteIds, separator, nextAliases, naming);
    }
  };

  const handleMoveSite = (siteId: string, direction: 'up' | 'down') => {
    const nextSiteIds = moveAggregateSite(siteIds, siteId, direction);
    setSiteIds(nextSiteIds);
    if (engaged && nextSiteIds.length > 0 && separatorError === null && normalizedAliases) {
      void runEngage(nextSiteIds, separator, normalizedAliases, naming);
    }
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
    const nextSiteIds = arrayMove(siteIds, oldIndex, newIndex);
    setSiteIds(nextSiteIds);
    if (engaged && nextSiteIds.length > 0 && separatorError === null && normalizedAliases) {
      void runEngage(nextSiteIds, separator, normalizedAliases, naming);
    }
  };

  const handleSeparatorCommit = () => {
    if (validateGatewayAggregateSeparator(separator) !== null) {
      return;
    }
    if (engaged && siteIds.length > 0 && normalizedAliases) {
      void runEngage(siteIds, separator, normalizedAliases, naming);
    }
  };

  const handleAliasCommit = () => {
    if (!engaged || siteIds.length === 0) {
      return;
    }
    const nextAliases = normalizeGatewayAggregateAliases(aliases, siteIds, candidateSiteIds);
    if (!nextAliases) {
      setNotice({ kind: 'error', text: t('gateway.aggregate.aliasInvalid') });
      return;
    }
    if (separatorError === null) {
      void runEngage(siteIds, separator, nextAliases, naming);
    }
  };

  const handleClearSelection = () => {
    if (engaged) {
      // Clearing the last aggregate site is equivalent to leaving aggregate
      // mode; keep the backend and the visible form in sync.
      void handleToggle(false);
      return;
    }
    setSiteIds([]);
  };

  const selectedCandidates = siteIds
    .map((siteId) => candidates.find((candidate) => candidate.id === siteId))
    .filter((candidate): candidate is GatewayAggregateSiteCandidate => Boolean(candidate));
  const unselectedCandidates = candidates.filter((candidate) => !siteIds.includes(candidate.id));
  const separatorExample = buildGatewayAggregateModelSlug(
    aliases[candidates[0]?.id ?? ''] || candidates[0]?.id || 'site-id',
    'model',
    separatorError === null ? separator : DEFAULT_AGGREGATE_SEPARATOR,
    naming,
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
            aria-label={t('gateway.aggregate.naming')}
            onChange={(event) => {
              const nextNaming = event.currentTarget.value as GatewayAggregateNamingMode;
              setNaming(nextNaming);
              if (engaged && normalizedAliases && siteIds.length > 0) {
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
            {siteIds.length > 0 ? (
              <button
                type="button"
                className={styles.textButton}
                onClick={handleClearSelection}
              >
                {t('gateway.aggregate.clearSelection')}
              </button>
            ) : (
              <button
                type="button"
                className={styles.textButton}
                onClick={() => setSiteIds(candidates.map((candidate) => candidate.id))}
              >
                {t('gateway.aggregate.selectAll')}
              </button>
            )}
          </div>

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
                  </span>
                  <code className={styles.siteSlug} title={candidate.id}>
                    {candidate.id}
                  </code>
                </li>
              ))}
            </ul>
          ) : null}
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
