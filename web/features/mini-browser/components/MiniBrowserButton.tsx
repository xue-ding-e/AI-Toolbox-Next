import React from 'react';
import { Button, Input, Modal, Popconfirm, Tooltip, Typography, message } from 'antd';
import { Globe, Plus, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  clearMiniBrowserProfile,
  closeMiniBrowser,
  closeMiniBrowserWindow,
  createMiniBrowserProfileId,
  focusMiniBrowserWindow,
  getMiniBrowserCurrentUrl,
  isMiniBrowserOpen,
  listMiniBrowserWindows,
  loadMiniBrowserAccounts,
  loadMiniBrowserSites,
  nextMiniBrowserAccountLabel,
  normaliseMiniBrowserUrl,
  openMiniBrowser,
  saveMiniBrowserAccounts,
  saveMiniBrowserSites,
  stableMiniBrowserProfileId,
  type MiniBrowserAccount,
  type MiniBrowserSite,
  type MiniBrowserWindowInfo,
} from '@/services/miniBrowserApi';
import styles from './MiniBrowserButton.module.less';

/**
 * Label for one open window.
 *
 * A window is normally backed by a saved account. If it is not — the legacy
 * address-input window, or an account that was deleted while its window stayed
 * open — fall back to something honest rather than an invented site name.
 */
const tabLabel = (
  windowInfo: MiniBrowserWindowInfo,
  entry: { account: MiniBrowserAccount; site?: MiniBrowserSite } | undefined,
  manualWindowLabel: string,
): string => {
  if (entry) return `${entry.site?.name ?? ''} · ${entry.account.label}`;
  return windowInfo.profile_id ?? manualWindowLabel;
};

/**
 * Header entry for the embedded browser.
 *
 * The window itself is a native webview created by the backend (see
 * `tauri/src/mini_browser.rs`). One relay can be logged into several times:
 * every saved account gets its own backend profile, so its window and its
 * cookies are independent and one login survives restarts.
 *
 * This component owns the saved sites/accounts, the open-tab strip, and the
 * legacy "type an address" entry point (which uses the shared no-profile
 * window and therefore shares one login).
 */
export const MiniBrowserButton: React.FC = () => {
  const { t } = useTranslation();
  const [open, setOpen] = React.useState(false);
  const [url, setUrl] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const [openWindows, setOpenWindows] = React.useState<MiniBrowserWindowInfo[]>([]);
  const [sites, setSites] = React.useState<MiniBrowserSite[]>(() => loadMiniBrowserSites());
  const [accounts, setAccounts] = React.useState<MiniBrowserAccount[]>(() =>
    loadMiniBrowserAccounts(),
  );
  const [siteName, setSiteName] = React.useState('');
  /** Site whose "new account" input is expanded, or null. */
  const [accountFormSiteId, setAccountFormSiteId] = React.useState<string | null>(null);
  const [accountName, setAccountName] = React.useState('');

  const accountsByProfile = React.useMemo(() => {
    const map = new Map<string, { account: MiniBrowserAccount; site?: MiniBrowserSite }>();
    for (const account of accounts) {
      map.set(account.id, {
        account,
        site: sites.find((site) => site.id === account.siteId),
      });
    }
    return map;
  }, [accounts, sites]);

  const windowsByProfile = React.useMemo(() => {
    const map = new Map<string, MiniBrowserWindowInfo>();
    for (const windowInfo of openWindows) {
      if (windowInfo.profile_id) map.set(windowInfo.profile_id, windowInfo);
    }
    return map;
  }, [openWindows]);

  // The browser windows live in the backend and can be closed from their own
  // title bars, so the open set is re-read when the panel opens and after every
  // action that changes it.
  const refresh = React.useCallback(async () => {
    try {
      const [windows, legacyOpen, legacyUrl] = await Promise.all([
        listMiniBrowserWindows(),
        isMiniBrowserOpen(),
        getMiniBrowserCurrentUrl(),
      ]);
      setOpenWindows(windows);
      // Only the legacy no-profile window prefills the legacy address input:
      // that input drives that window, not any account's.
      if (legacyOpen && legacyUrl) {
        setUrl((previous) => (previous ? previous : legacyUrl));
      }
    } catch {
      // Reading state is best-effort: the panel still works without it.
    }
  }, []);

  const persistSites = React.useCallback((next: MiniBrowserSite[]) => {
    setSites(next);
    saveMiniBrowserSites(next);
  }, []);

  const persistAccounts = React.useCallback((next: MiniBrowserAccount[]) => {
    setAccounts(next);
    saveMiniBrowserAccounts(next);
  }, []);

  /** Open a URL and refresh the tab strip; reports failures in one place. */
  const openUrl = React.useCallback(
    async (targetUrl: string, profileId?: string): Promise<boolean> => {
      try {
        await openMiniBrowser(targetUrl, profileId);
        await refresh();
        return true;
      } catch (error) {
        void message.error(
          t('miniBrowser.openFailed', {
            error: error instanceof Error ? error.message : String(error),
          }),
        );
        return false;
      }
    },
    [refresh, t],
  );

  const handleOpen = async () => {
    const normalised = normaliseMiniBrowserUrl(url);
    if (!normalised) {
      void message.warning(
        url.trim() ? t('miniBrowser.urlInvalid') : t('miniBrowser.urlRequired'),
      );
      return;
    }
    setBusy(true);
    try {
      await openUrl(normalised);
    } finally {
      setBusy(false);
    }
  };

  const handleOpenAllAccounts = async () => {
    if (accounts.length === 0) {
      void message.warning(t('miniBrowser.noAccounts'));
      return;
    }
    setBusy(true);
    try {
      let failed = 0;
      // Sequential on purpose: every call creates or focuses a native window,
      // and firing them in parallel makes the final focus order arbitrary.
      for (const account of accounts) {
        const site = sites.find((candidate) => candidate.id === account.siteId);
        if (!site) continue;
        if (!(await openUrl(site.url, account.id))) failed += 1;
      }
      if (failed === 0) void message.success(t('miniBrowser.openAllDone'));
    } finally {
      setBusy(false);
    }
  };

  /** Create a site plus its first default account. */
  const handleSaveSite = (rawUrl: string, name: string) => {
    const normalised = normaliseMiniBrowserUrl(rawUrl);
    if (!normalised || !name.trim()) {
      void message.warning(t('miniBrowser.siteRequired'));
      return false;
    }

    // Saving a URL that is already listed renames it instead of replacing it:
    // replacing would orphan the accounts (and keep their logins on disk with
    // nothing pointing at them).
    const existingSite = sites.find((candidate) => candidate.url === normalised);
    if (existingSite) {
      persistSites(
        sites.map((candidate) =>
          candidate.id === existingSite.id ? { ...candidate, name: name.trim() } : candidate,
        ),
      );
      void message.success(t('miniBrowser.siteSaved'));
      return true;
    }

    const id = createMiniBrowserProfileId();
    const site: MiniBrowserSite = { id, name: name.trim(), url: normalised };
    const nextSites = [...sites, site];
    const nextAccounts = [
      ...accounts,
      { id: stableMiniBrowserProfileId(id), siteId: id, label: site.name },
    ].filter((account) => nextSites.some((candidate) => candidate.id === account.siteId));
    persistSites(nextSites);
    persistAccounts(nextAccounts);
    void message.success(t('miniBrowser.siteSaved'));
    return true;
  };

  const handleAddSiteFromInput = () => {
    if (handleSaveSite(url, siteName)) {
      setSiteName('');
      setUrl('');
    }
  };

  const handleDeleteSite = async (siteId: string) => {
    const removed = accounts.filter((account) => account.siteId === siteId);
    persistSites(sites.filter((site) => site.id !== siteId));
    persistAccounts(accounts.filter((account) => account.siteId !== siteId));
    // Deleting a site also forgets the login state of its accounts.
    for (const account of removed) {
      await clearMiniBrowserProfile(account.id).catch(() => undefined);
    }
    await refresh();
  };

  const handleAddAccount = (site: MiniBrowserSite) => {
    const label = accountName.trim();
    if (!label) {
      void message.warning(t('miniBrowser.accountNameRequired'));
      return;
    }
    persistAccounts([
      ...accounts,
      { id: createMiniBrowserProfileId(), siteId: site.id, label },
    ]);
    setAccountFormSiteId(null);
    setAccountName('');
  };

  const handleDeleteAccount = async (account: MiniBrowserAccount) => {
    persistAccounts(accounts.filter((candidate) => candidate.id !== account.id));
    // Closing the window and dropping its data directory is what actually
    // forgets the login; the account entry itself is already gone locally.
    await clearMiniBrowserProfile(account.id).catch(() => undefined);
    await refresh();
  };

  const handleCloseWindow = async (profileId: string) => {
    await closeMiniBrowserWindow(profileId).catch(() => undefined);
    await refresh();
  };

  const handleCloseAll = async () => {
    await closeMiniBrowser().catch(() => undefined);
    await refresh();
  };

  return (
    <>
      <Tooltip title={t('miniBrowser.tooltip')}>
        <span className={styles.button} role="button" tabIndex={0}
          onClick={() => {
            setOpen(true);
            void refresh();
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault();
              setOpen(true);
              void refresh();
            }
          }}
        >
          <Globe className={styles.icon} size={14} />
          <span className={styles.text}>{t('miniBrowser.button')}</span>
        </span>
      </Tooltip>
      <Modal
        open={open}
        title={t('miniBrowser.title')}
        onCancel={() => setOpen(false)}
        destroyOnHidden
        width={680}
        footer={[
          openWindows.length > 0 ? (
            <Button
              key="close-all"
              onClick={() => void handleCloseAll()}
            >
              {t('miniBrowser.closeAllWindows')}
            </Button>
          ) : null,
          <Button key="cancel" onClick={() => setOpen(false)}>
            {t('miniBrowser.cancel')}
          </Button>,
          <Button key="open-all" loading={busy} onClick={() => void handleOpenAllAccounts()}>
            {t('miniBrowser.openAllAccounts')}
          </Button>,
        ]}
      >
        <section className={styles.section}>
          <Typography.Text strong className={styles.sectionTitle}>
            {t('miniBrowser.openTabs')}
          </Typography.Text>
          {openWindows.length > 0 ? (
            <div className={styles.tabStrip}>
              {openWindows.map((windowInfo) => {
                const entry = windowInfo.profile_id
                  ? accountsByProfile.get(windowInfo.profile_id)
                  : undefined;
                return (
                  <div className={styles.tab} key={windowInfo.label}>
                    <button
                      type="button"
                      className={styles.tabMain}
                      title={windowInfo.url}
                      onClick={
                        windowInfo.profile_id
                          ? () => {
                              void focusMiniBrowserWindow(windowInfo.profile_id as string).catch(
                                () => undefined,
                              );
                            }
                          : undefined
                      }
                    >
                      <span className={styles.tabName}>
                        {tabLabel(windowInfo, entry, t('miniBrowser.manualWindow'))}
                      </span>
                      <span className={styles.tabUrl}>{windowInfo.url}</span>
                    </button>
                    {/*
                      The address-input window has no profile id, and the only
                      backend close command that reaches it also closes every
                      account window. Its tab therefore offers no per-tab close;
                      use the "close all" button instead.
                    */}
                    {windowInfo.profile_id ? (
                      <button
                        type="button"
                        className={styles.tabClose}
                        aria-label={t('miniBrowser.closeTab')}
                        title={t('miniBrowser.closeTab')}
                        onClick={() => void handleCloseWindow(windowInfo.profile_id as string)}
                      >
                        <X size={12} />
                      </button>
                    ) : null}
                  </div>
                );
              })}
            </div>
          ) : (
            <Typography.Paragraph type="secondary" className={styles.emptyHint}>
              {t('miniBrowser.noOpenTabs')}
            </Typography.Paragraph>
          )}
        </section>

        <section className={styles.section}>
          <Typography.Text strong className={styles.sectionTitle}>
            {t('miniBrowser.siteSection')}
          </Typography.Text>
          {sites.length > 0 ? (
            <div className={styles.siteList}>
              {sites.map((site) => {
                const siteAccounts = accounts.filter((account) => account.siteId === site.id);
                return (
                  <div className={styles.siteBlock} key={site.id}>
                    <div className={styles.siteHeader}>
                      <div className={styles.siteTitle}>
                        <strong>{site.name}</strong>
                        <span>{site.url}</span>
                      </div>
                      <div className={styles.siteActions}>
                        <Button
                          size="small"
                          onClick={() => {
                            setAccountFormSiteId(site.id);
                            setAccountName(nextMiniBrowserAccountLabel(site, accounts));
                          }}
                        >
                          <Plus size={12} /> {t('miniBrowser.addAccount')}
                        </Button>
                        <Button
                          size="small"
                          onClick={() => void openUrl(site.url, siteAccounts[0]?.id)}
                        >
                          {t('miniBrowser.openSiteDefault')}
                        </Button>
                        <Popconfirm
                          title={t('miniBrowser.removeSiteConfirm')}
                          okText={t('miniBrowser.removeSite')}
                          cancelText={t('miniBrowser.cancel')}
                          onConfirm={() => void handleDeleteSite(site.id)}
                        >
                          <Button size="small" danger>
                            {t('miniBrowser.removeSite')}
                          </Button>
                        </Popconfirm>
                      </div>
                    </div>

                    <div className={styles.accountList}>
                      {siteAccounts.map((account) => {
                        const windowInfo = windowsByProfile.get(account.id);
                        return (
                          <div className={styles.accountRow} key={account.id}>
                            <div className={styles.accountInfo}>
                              <Input
                                size="small"
                                className={styles.accountName}
                                value={account.label}
                                aria-label={t('miniBrowser.accountName')}
                                onChange={(event) => {
                                  const label = event.currentTarget.value;
                                  persistAccounts(
                                    accounts.map((candidate) =>
                                      candidate.id === account.id
                                        ? { ...candidate, label }
                                        : candidate,
                                    ),
                                  );
                                }}
                              />
                              <span
                                className={
                                  windowInfo ? styles.statusOpen : styles.statusClosed
                                }
                              >
                                {windowInfo
                                  ? t('miniBrowser.statusOpened')
                                  : t('miniBrowser.statusClosed')}
                              </span>
                            </div>
                            <div className={styles.accountMeta}>
                              <span className={styles.accountUrl}>
                                {windowInfo?.url || t('miniBrowser.noWindow')}
                              </span>
                              <div className={styles.accountActions}>
                                <Button
                                  size="small"
                                  type="primary"
                                  onClick={() => void openUrl(site.url, account.id)}
                                >
                                  {windowInfo
                                    ? t('miniBrowser.focusAccount')
                                    : t('miniBrowser.openAccount')}
                                </Button>
                                <Popconfirm
                                  title={t('miniBrowser.removeAccountConfirm')}
                                  okText={t('miniBrowser.removeAccount')}
                                  cancelText={t('miniBrowser.cancel')}
                                  onConfirm={() => void handleDeleteAccount(account)}
                                >
                                  <Button size="small" danger>
                                    {t('miniBrowser.removeAccount')}
                                  </Button>
                                </Popconfirm>
                              </div>
                            </div>
                          </div>
                        );
                      })}
                      {accountFormSiteId === site.id ? (
                        <div className={styles.accountForm}>
                          <Input
                            autoFocus
                            size="small"
                            value={accountName}
                            placeholder={t('miniBrowser.accountNamePlaceholder')}
                            aria-label={t('miniBrowser.accountName')}
                            onChange={(event) => setAccountName(event.currentTarget.value)}
                            onPressEnter={() => handleAddAccount(site)}
                          />
                          <Button size="small" type="primary" onClick={() => handleAddAccount(site)}>
                            {t('miniBrowser.confirmAddAccount')}
                          </Button>
                          <Button
                            size="small"
                            onClick={() => {
                              setAccountFormSiteId(null);
                              setAccountName('');
                            }}
                          >
                            {t('miniBrowser.cancel')}
                          </Button>
                        </div>
                      ) : null}
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <Typography.Paragraph type="secondary" className={styles.emptyHint}>
              {t('miniBrowser.noSites')}
            </Typography.Paragraph>
          )}
        </section>

        <Typography.Paragraph type="secondary" className={styles.hint}>
          {t('miniBrowser.hint')}
        </Typography.Paragraph>

        <section className={styles.section}>
          <Typography.Text strong className={styles.sectionTitle}>
            {t('miniBrowser.manualSection')}
          </Typography.Text>
          <div className={styles.manualRow}>
            <Input
              value={url}
              placeholder={t('miniBrowser.urlPlaceholder')}
              aria-label={t('miniBrowser.urlLabel')}
              onChange={(event) => setUrl(event.currentTarget.value)}
              onPressEnter={() => void handleOpen()}
            />
            <Button type="primary" loading={busy} onClick={() => void handleOpen()}>
              {t('miniBrowser.open')}
            </Button>
          </div>
          <div className={styles.saveSiteRow}>
            <Input
              value={siteName}
              placeholder={t('miniBrowser.siteNamePlaceholder')}
              aria-label={t('miniBrowser.siteName')}
              onChange={(event) => setSiteName(event.currentTarget.value)}
            />
            <Button onClick={handleAddSiteFromInput}>{t('miniBrowser.saveSite')}</Button>
          </div>
        </section>
      </Modal>
    </>
  );
};

export default MiniBrowserButton;
