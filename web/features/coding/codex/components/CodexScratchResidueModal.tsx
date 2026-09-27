import React from 'react';
import { Alert, App, Button, Checkbox, Empty, Modal, Spin, Tag, Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import type {
  CodexHistorySourceMode,
  CodexScratchCleanResult,
  CodexScratchResidue,
} from '@/types/codex';
import { cleanCodexScratchResidue, scanCodexScratchResidue } from '@/services/codexApi';

import {
  defaultResidueSelection,
  describeResidueCleanOutcome,
  emptyResidueSelection,
  isResidueSelectionEmpty,
  isWorkspaceRemovable,
  residueSelectionCount,
  setResidueGroupSelection,
  summarizeResidueSelection,
  toggleResidueSelection,
  type CodexScratchResidueSelection,
} from '../utils/codexScratchResidueSelection';
import styles from './CodexScratchResidueModal.module.less';

const { Text } = Typography;

interface CodexScratchResidueModalProps {
  open: boolean;
  onClose: () => void;
  /** The source the session panel is showing, so both views stay aligned. */
  sourceMode: CodexHistorySourceMode;
}

const formatSize = (bytes: number): string => {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

/**
 * Residue of Codex project-less chats: the generated workspace directories and
 * `[projects]` trust entries whose sessions are gone.
 *
 * Nothing here is inferred on the frontend: the backend decides what counts as
 * residue (no rollout references it), and it re-checks every item when the
 * cleanup runs, so a stale dialog cannot delete a directory that came back into
 * use.
 */
const CodexScratchResidueModal: React.FC<CodexScratchResidueModalProps> = ({
  open,
  onClose,
  sourceMode,
}) => {
  const { t } = useTranslation();
  const { message, modal } = App.useApp();
  const [report, setReport] = React.useState<CodexScratchResidue | null>(null);
  const [selection, setSelection] = React.useState<CodexScratchResidueSelection>(
    emptyResidueSelection,
  );
  const [loading, setLoading] = React.useState(false);
  const [cleaning, setCleaning] = React.useState(false);
  const [lastResult, setLastResult] = React.useState<CodexScratchCleanResult | null>(null);
  const requestIdRef = React.useRef(0);

  const loadResidue = React.useCallback(async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    setLoading(true);
    try {
      const next = await scanCodexScratchResidue(sourceMode);
      if (requestIdRef.current !== requestId) {
        return;
      }
      setReport(next);
      setSelection(defaultResidueSelection(next));
    } catch (error) {
      if (requestIdRef.current !== requestId) {
        return;
      }
      const errorMessage = error instanceof Error ? error.message : String(error);
      message.error(errorMessage || t('common.error'));
      setReport(null);
      setSelection(emptyResidueSelection());
    } finally {
      if (requestIdRef.current === requestId) {
        setLoading(false);
      }
    }
  }, [sourceMode, t]);

  React.useEffect(() => {
    if (!open) {
      return;
    }
    setLastResult(null);
    void loadResidue();
  }, [loadResidue, open]);

  const handleClean = () => {
    if (!report || isResidueSelectionEmpty(selection)) {
      return;
    }

    const summary = summarizeResidueSelection(selection, report);
    modal.confirm({
      title: t('codex.scratchResidue.confirmTitle'),
      content: (
        <div className={styles.confirmBody}>
          <div>{t('codex.scratchResidue.confirmContent', { ...summary })}</div>
          <Text className={styles.helperText}>{t('codex.scratchResidue.confirmHint')}</Text>
        </div>
      ),
      okText: t('codex.scratchResidue.clean'),
      okButtonProps: { danger: true },
      cancelText: t('common.cancel'),
      onOk: async () => {
        const requestId = requestIdRef.current + 1;
        requestIdRef.current = requestId;
        setCleaning(true);
        try {
          const result = await cleanCodexScratchResidue(
            sourceMode,
            selection.workspaces,
            selection.trustKeys,
            selection.dateDirs,
          );
          if (requestIdRef.current !== requestId) {
            return;
          }
          setLastResult(result);

          const outcome = describeResidueCleanOutcome(result);
          if (outcome) {
            const text = t(outcome.key, outcome.params);
            if (outcome.tone === 'warning') {
              message.warning(text);
            } else {
              message.success(text);
            }
          }
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : String(error);
          message.error(errorMessage || t('common.error'));
        } finally {
          setCleaning(false);
          await loadResidue();
        }
      },
    });
  };

  const removableWorkspaces = report?.workspaces.filter(isWorkspaceRemovable) ?? [];
  const blockedWorkspaces = report?.workspaces.filter((workspace) => workspace.info.hasGit) ?? [];
  const hasItems =
    removableWorkspaces.length > 0 ||
    blockedWorkspaces.length > 0 ||
    (report?.trustEntries.length ?? 0) > 0 ||
    (report?.emptyDateDirs.length ?? 0) > 0;
  const scanComplete = report?.scanComplete ?? false;

  const renderSourceTag = (runtimeSource: 'local' | 'wsl', distro?: string | null) => (
    <Tag className={styles.sourceTag}>
      {runtimeSource === 'wsl'
        ? distro
          ? t('sessionManager.sourceMode.wslWithDistro', { distro })
          : t('sessionManager.sourceMode.wsl')
        : t('sessionManager.sourceMode.local')}
    </Tag>
  );

  return (
    <Modal
      open={open}
      onCancel={onClose}
      title={t('codex.scratchResidue.title')}
      width={720}
      destroyOnHidden
      footer={[
        <Button key="close" onClick={onClose}>
          {t('common.close')}
        </Button>,
        <Button
          key="clean"
          danger
          loading={cleaning}
          disabled={!scanComplete || isResidueSelectionEmpty(selection)}
          onClick={handleClean}
        >
          {t('codex.scratchResidue.cleanSelected', {
            count: residueSelectionCount(selection),
          })}
        </Button>,
      ]}
    >
      <Spin spinning={loading}>
        <div className={styles.body}>
          <Text className={styles.introText}>{t('codex.scratchResidue.description')}</Text>

          {report?.unavailable ? (
            <Empty description={t('codex.scratchResidue.unavailable')} />
          ) : null}

          {report && !report.unavailable && !scanComplete ? (
            <Alert
              type="warning"
              showIcon
              message={t('codex.scratchResidue.scanIncomplete')}
            />
          ) : null}

          {report && !report.unavailable && hasItems ? (
            <>
              {removableWorkspaces.length > 0 ? (
                <section className={styles.group}>
                  <div className={styles.groupHeader}>
                    <Text strong>{t('codex.scratchResidue.workspacesTitle')}</Text>
                    <Button
                      type="link"
                      size="small"
                      className={styles.groupAction}
                      disabled={loading}
                      onClick={() =>
                        setResidueGroupSelection(
                          selection,
                          'workspace',
                          removableWorkspaces.map((workspace) => workspace.info.path),
                        )
                      }
                    >
                      {t('codex.scratchResidue.selectAll')}
                    </Button>
                    <Button
                      type="link"
                      size="small"
                      className={styles.groupAction}
                      disabled={loading}
                      onClick={() => setResidueGroupSelection(selection, 'workspace', [])}
                    >
                      {t('codex.scratchResidue.selectNone')}
                    </Button>
                  </div>
                  {removableWorkspaces.map((workspace) => (
                    <div key={workspace.info.path} className={styles.item}>
                      <Checkbox
                        checked={selection.workspaces.includes(workspace.info.path)}
                        onChange={(event) =>
                          setSelection((current) =>
                            toggleResidueSelection(
                              current,
                              'workspace',
                              workspace.info.path,
                              event.target.checked,
                            ),
                          )
                        }
                      >
                        <span className={styles.itemPath}>{workspace.info.path}</span>
                      </Checkbox>
                      <Text className={styles.itemDetail}>
                        {workspace.info.isEmpty
                          ? t('codex.scratchResidue.workspaceEmpty')
                          : t('codex.scratchResidue.workspaceFiles', {
                              count: workspace.info.fileCount,
                              size: formatSize(workspace.info.totalBytes),
                            })}
                        {renderSourceTag(workspace.runtimeSource, workspace.runtimeDistro)}
                      </Text>
                    </div>
                  ))}
                </section>
              ) : null}

              {blockedWorkspaces.length > 0 ? (
                <section className={styles.group}>
                  <Text strong>{t('codex.scratchResidue.blockedTitle')}</Text>
                  {blockedWorkspaces.map((workspace) => (
                    <div key={workspace.info.path} className={styles.item}>
                      <Checkbox disabled>{workspace.info.path}</Checkbox>
                      <Text className={styles.itemDetail}>
                        {t('codex.scratchResidue.workspaceGit')}
                      </Text>
                    </div>
                  ))}
                </section>
              ) : null}

              {report.trustEntries.length > 0 ? (
                <section className={styles.group}>
                  <div className={styles.groupHeader}>
                    <Text strong>{t('codex.scratchResidue.trustTitle')}</Text>
                    <Button
                      type="link"
                      size="small"
                      className={styles.groupAction}
                      disabled={loading}
                      onClick={() =>
                        setResidueGroupSelection(
                          selection,
                          'trustKey',
                          report.trustEntries.map((entry) => entry.key),
                        )
                      }
                    >
                      {t('codex.scratchResidue.selectAll')}
                    </Button>
                    <Button
                      type="link"
                      size="small"
                      className={styles.groupAction}
                      disabled={loading}
                      onClick={() => setResidueGroupSelection(selection, 'trustKey', [])}
                    >
                      {t('codex.scratchResidue.selectNone')}
                    </Button>
                  </div>
                  {report.trustEntries.map((entry) => (
                    <div key={entry.key} className={styles.item}>
                      <Checkbox
                        checked={selection.trustKeys.includes(entry.key)}
                        onChange={(event) =>
                          setSelection((current) =>
                            toggleResidueSelection(
                              current,
                              'trustKey',
                              entry.key,
                              event.target.checked,
                            ),
                          )
                        }
                      >
                        <span className={styles.itemPath}>{entry.key}</span>
                      </Checkbox>
                      <Text className={styles.itemDetail}>
                        {entry.dirExists
                          ? t('codex.scratchResidue.trustDirExists')
                          : t('codex.scratchResidue.trustDirMissing')}
                        {renderSourceTag(entry.runtimeSource, entry.runtimeDistro)}
                      </Text>
                    </div>
                  ))}
                </section>
              ) : null}

              {report.emptyDateDirs.length > 0 ? (
                <section className={styles.group}>
                  <Text strong>{t('codex.scratchResidue.dateDirsTitle')}</Text>
                  {report.emptyDateDirs.map((path) => (
                    <div key={path} className={styles.item}>
                      <Checkbox
                        checked={selection.dateDirs.includes(path)}
                        onChange={(event) =>
                          setSelection((current) =>
                            toggleResidueSelection(
                              current,
                              'dateDir',
                              path,
                              event.target.checked,
                            ),
                          )
                        }
                      >
                        <span className={styles.itemPath}>{path}</span>
                      </Checkbox>
                    </div>
                  ))}
                </section>
              ) : null}
            </>
          ) : null}

          {report && !report.unavailable && scanComplete && !hasItems ? (
            <Empty description={t('codex.scratchResidue.empty')} />
          ) : null}

          {lastResult && (lastResult.failures.length > 0 || lastResult.skipped.length > 0) ? (
            <Alert
              type="warning"
              showIcon
              message={t('codex.scratchResidue.partialHint')}
              description={
                <div className={styles.detailList}>
                  {lastResult.failures.map((failure) => (
                    <div key={`failure-${failure.target}`}>
                      {t('codex.scratchResidue.failureItem', {
                        target: failure.target,
                        error: failure.error,
                      })}
                    </div>
                  ))}
                  {lastResult.skipped.map((skip) => (
                    <div key={`skip-${skip.target}`}>
                      {t('codex.scratchResidue.skipItem', {
                        target: skip.target,
                        reason: skip.reason,
                      })}
                    </div>
                  ))}
                </div>
              }
            />
          ) : null}
        </div>
      </Spin>
    </Modal>
  );
};

export default CodexScratchResidueModal;