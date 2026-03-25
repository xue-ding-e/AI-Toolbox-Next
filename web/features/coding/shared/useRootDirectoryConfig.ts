import React from 'react';
import type { TFunction } from 'i18next';

type RootPathSource = 'custom' | 'env' | 'shell' | 'default';

export interface RootPathInfoLike {
  path: string;
  source: RootPathSource;
}

interface CommonConfigLike {
  config?: string;
  rootDir?: string | null;
}

interface SaveRootDirectoryInput {
  config: string;
  rootDir?: string | null;
  clearRootDir?: boolean;
}

interface UseRootDirectoryConfigOptions<TCommonConfig extends CommonConfigLike> {
  t: TFunction;
  translationKeyPrefix: string;
  defaultConfig: string;
  loadConfig: (silent?: boolean) => Promise<void>;
  getCommonConfig: () => Promise<TCommonConfig | null>;
  saveCommonConfig: (input: SaveRootDirectoryInput) => Promise<void>;
}

export const getRootPathSourceLabel = (
  t: TFunction,
  translationKeyPrefix: string,
  pathInfo: RootPathInfoLike | null,
): string => {
  if (!pathInfo) {
    return '';
  }

  switch (pathInfo.source) {
    case 'custom':
      return t(`${translationKeyPrefix}.rootPathSource.modal.sourceCustom`);
    case 'env':
      return t(`${translationKeyPrefix}.rootPathSource.modal.sourceEnv`);
    case 'shell':
      return t(`${translationKeyPrefix}.rootPathSource.modal.sourceShell`);
    default:
      return t(`${translationKeyPrefix}.rootPathSource.modal.sourceDefault`);
  }
};

const useRootDirectoryConfig = <TCommonConfig extends CommonConfigLike>({
  t,
  translationKeyPrefix,
  defaultConfig,
  loadConfig,
  getCommonConfig,
  saveCommonConfig,
}: UseRootDirectoryConfigOptions<TCommonConfig>) => {
  const [rootDirectoryModalOpen, setRootDirectoryModalOpen] = React.useState(false);

  const getSourceLabel = React.useCallback(
    (pathInfo: RootPathInfoLike | null) =>
      getRootPathSourceLabel(t, translationKeyPrefix, pathInfo),
    [t, translationKeyPrefix],
  );

  const getRootDirectoryModalProps = React.useCallback(
    (pathInfo: RootPathInfoLike | null) => ({
      title: t(`${translationKeyPrefix}.rootPathSource.modal.title`),
      currentPathInfo: pathInfo,
      currentSourceText: t(`${translationKeyPrefix}.rootPathSource.modal.currentSource`),
      currentSourceLabel: getSourceLabel(pathInfo),
      description: t(`${translationKeyPrefix}.rootPathSource.modal.description`),
      envWarningTitle: t(`${translationKeyPrefix}.rootPathSource.modal.envWarningTitle`),
      envWarningDesc: t(`${translationKeyPrefix}.rootPathSource.modal.envWarningDesc`),
      envHint:
        pathInfo?.source === 'env' || pathInfo?.source === 'shell'
          ? t(`${translationKeyPrefix}.rootPathSource.modal.${pathInfo.source}Hint`, {
              path: pathInfo.path,
            })
          : null,
      customPathLabel: t(`${translationKeyPrefix}.rootPathSource.modal.customPath`),
      placeholder: t(`${translationKeyPrefix}.rootPathSource.modal.placeholder`),
      selectFolderText: t('common.selectFolder'),
      folderOnlyTitle: t(`${translationKeyPrefix}.rootPathSource.modal.folderOnlyTitle`),
      folderOnlyDesc: t(`${translationKeyPrefix}.rootPathSource.modal.folderOnlyDesc`),
      saveSuccessText: t(`${translationKeyPrefix}.rootPathSource.modal.saveSuccess`),
      resetSuccessText: t(`${translationKeyPrefix}.rootPathSource.modal.resetSuccess`),
      resetText: t(`${translationKeyPrefix}.rootPathSource.modal.reset`),
      cancelText: t('common.cancel'),
      saveText: t('common.save'),
      errorText: t('common.error'),
    }),
    [getSourceLabel, t, translationKeyPrefix],
  );

  const handleSaveRootDirectory = React.useCallback(
    async (rootDir: string | null) => {
      const currentCommonConfig = await getCommonConfig();
      await saveCommonConfig({
        config: currentCommonConfig?.config || defaultConfig,
        rootDir,
        clearRootDir: rootDir === null,
      });
      setRootDirectoryModalOpen(false);
      await loadConfig(true);
    },
    [defaultConfig, getCommonConfig, loadConfig, saveCommonConfig],
  );

  const handleResetRootDirectory = React.useCallback(async () => {
    const currentCommonConfig = await getCommonConfig();
    await saveCommonConfig({
      config: currentCommonConfig?.config || defaultConfig,
      rootDir: null,
      clearRootDir: true,
    });
    setRootDirectoryModalOpen(false);
    await loadConfig(true);
  }, [defaultConfig, getCommonConfig, loadConfig, saveCommonConfig]);

  return {
    rootDirectoryModalOpen,
    setRootDirectoryModalOpen,
    getSourceLabel,
    getRootDirectoryModalProps,
    handleSaveRootDirectory,
    handleResetRootDirectory,
  };
};

export default useRootDirectoryConfig;
