import { emit, listen } from "@tauri-apps/api/event";
import {
	App,
	theme as antdTheme,
	Button,
	ConfigProvider,
	Modal,
	Progress,
	Space,
	Spin,
	Typography,
} from "antd";
import enUS from "antd/locale/en_US";
import zhCN from "antd/locale/zh_CN";
import React from "react";
import i18n from "@/i18n";
import {
	checkForUpdates,
	fetchRemotePresetModels,
	GITHUB_REPO,
	installUpdate,
	loadCachedPresetModels,
	openExternalUrl,
	refreshTrayMenu,
	setWindowBackgroundColor,
	type UpdateInfo,
} from "@/services";
import { restartApp } from "@/services/settingsApi";
import { useAppStore, useSettingsStore } from "@/stores";
import { useThemeStore } from "@/stores/themeStore";

interface ProvidersProps {
	children: React.ReactNode;
}

const antdLocales = {
	"zh-CN": zhCN,
	"en-US": enUS,
};

const { Text } = Typography;

const KB = 1024;

function formatSiUnit(
	value: number,
	units: string[],
	zeroLabel: string,
): string {
	if (value === 0) return zeroLabel;
	const i = Math.floor(Math.log(value) / Math.log(KB));
	return `${parseFloat((value / KB ** i).toFixed(2))} ${units[i]}`;
}

const formatFileSize = (bytes: number) =>
	formatSiUnit(bytes, ["B", "KB", "MB", "GB"], "0 B");

const formatSpeed = (bytesPerSecond: number) =>
	formatSiUnit(bytesPerSecond, ["B/s", "KB/s", "MB/s", "GB/s"], "0 B/s");

const AppInitializer: React.FC<{ children: React.ReactNode }> = ({
	children,
}) => {
	const { notification, message } = App.useApp();
	const hasCheckedUpdate = React.useRef(false);

	const [updateModalOpen, setUpdateModalOpen] = React.useState(false);
	const [updateProgress, setUpdateProgress] = React.useState<number>(0);
	const [updateStatus, setUpdateStatus] = React.useState<string>("");
	const [updateSpeed, setUpdateSpeed] = React.useState<number>(0);
	const [updateDownloaded, setUpdateDownloaded] = React.useState<number>(0);
	const [updateTotal, setUpdateTotal] = React.useState<number>(0);

	React.useEffect(() => {
		const unlisten = listen<{
			status: string;
			progress: number;
			downloaded: number;
			total: number;
			speed: number;
		}>("update-download-progress", (event) => {
			const { status, progress, downloaded, total, speed } = event.payload;
			setUpdateStatus(status);
			setUpdateProgress(progress);
			setUpdateSpeed(speed);
			setUpdateDownloaded(downloaded);
			setUpdateTotal(total);

			if (status === "installing") {
				message.success(i18n.t("settings.about.downloadingComplete"));
			}
		});

		return () => {
			unlisten.then((fn) => fn()).catch(console.error);
		};
	}, [message]);

	const handleInstallUpdate = React.useCallback(
		async (info: UpdateInfo) => {
			notification.destroy();

			if (info.signature && info.url) {
				setUpdateModalOpen(true);
				setUpdateProgress(0);
				setUpdateStatus("started");
				setUpdateSpeed(0);
				setUpdateDownloaded(0);
				setUpdateTotal(0);

				try {
					await installUpdate();
					setUpdateModalOpen(false);
					Modal.success({
						title: i18n.t("settings.about.updateComplete"),
						content: i18n.t("settings.about.updateCompleteRestart"),
						okText: i18n.t("common.restart"),
						onOk: () => {
							void restartApp();
						},
					});
				} catch (error) {
					console.error("Failed to install update:", error);
					setUpdateModalOpen(false);

					const githubActionsUrl = `https://github.com/${GITHUB_REPO}/actions`;
					Modal.error({
						title: i18n.t("settings.about.updateFailed"),
						content: (
							<div>
								<p>{i18n.t("settings.about.updateFailedMessage")}</p>
								<p style={{ marginTop: 8 }}>
									<Typography.Link
										onClick={() => openExternalUrl(githubActionsUrl)}
									>
										{i18n.t("settings.about.goToGitHubActions")}
									</Typography.Link>
								</p>
							</div>
						),
						okText: i18n.t("common.close"),
					});
				}
			} else if (info.releaseUrl) {
				try {
					await openExternalUrl(info.releaseUrl);
				} catch (error) {
					console.error("Failed to open release page:", error);
				}
			}
		},
		[notification],
	);

	React.useEffect(() => {
		if (hasCheckedUpdate.current) return;
		hasCheckedUpdate.current = true;

		const LAST_CHECK_KEY = "lastUpdateCheckTime";
		const now = Date.now();
		const lastCheck = Number(localStorage.getItem(LAST_CHECK_KEY) || "0");
		// Skip rate limit in dev mode
		if (!import.meta.env.DEV && now - lastCheck < 3600000) return;

		const checkUpdate = async () => {
			try {
				const info = await checkForUpdates();
				localStorage.setItem(LAST_CHECK_KEY, String(now));
				if (info.hasUpdate) {
					notification.info({
						message: i18n.t("settings.about.newVersion"),
						description: i18n.t("settings.about.updateAvailable", {
							version: info.latestVersion,
						}),
						btn: (
							<Space>
								<Button
									size="small"
									onClick={() => {
										openExternalUrl(info.releaseUrl);
										notification.destroy();
									}}
								>
									{i18n.t("settings.about.viewReleaseNotes")}
								</Button>
								<Button
									type="primary"
									size="small"
									onClick={() => handleInstallUpdate(info)}
								>
									{i18n.t("settings.about.goToDownload")}
								</Button>
							</Space>
						),
						duration: 10,
					});
				}
			} catch (error) {
				console.error("Auto check update failed:", error);
			}
		};

		checkUpdate();
	}, [notification, handleInstallUpdate]);

	React.useEffect(() => {
		let unlisten: (() => void) | undefined;

		const setupListener = async () => {
			try {
				unlisten = await listen<string>("config-changed", (event) => {
					if (event.payload === "tray") {
						window.location.reload();
					}
				});
			} catch (error) {
				console.error("Failed to setup config change listener:", error);
			}
		};

		void setupListener();

		return () => {
			unlisten?.();
		};
	}, []);

	return (
		<>
			{children}
			{/* Update Progress Modal */}
			<Modal
				title={i18n.t("settings.about.downloadingUpdate")}
				open={updateModalOpen}
				closable={false}
				footer={null}
				centered
			>
				<div style={{ padding: "20px 0" }}>
					<Progress
						percent={updateProgress}
						status="active"
						strokeColor={{
							"0%": "#108ee9",
							"100%": "#87d068",
						}}
					/>
					<div style={{ marginTop: 16 }}>
						{updateStatus === "downloading" && (
							<div
								style={{
									display: "flex",
									justifyContent: "space-between",
									alignItems: "center",
								}}
							>
								<Text type="secondary" style={{ fontSize: 14 }}>
									{formatFileSize(updateDownloaded)} /{" "}
									{formatFileSize(updateTotal)}
								</Text>
								<Text
									style={{ color: "#1890ff", fontSize: 14, fontWeight: 500 }}
								>
									{formatSpeed(updateSpeed)}
								</Text>
							</div>
						)}
						{updateStatus === "installing" && (
							<Text type="secondary" style={{ fontSize: 14 }}>
								{i18n.t("settings.about.installingUpdate")}
							</Text>
						)}
						{updateStatus === "started" && (
							<Text type="secondary" style={{ fontSize: 14 }}>
								{i18n.t("settings.about.downloadingUpdate")}
							</Text>
						)}
					</div>
				</div>
			</Modal>
		</>
	);
};

export const Providers: React.FC<ProvidersProps> = ({ children }) => {
	const { language, isInitialized: appInitialized, initApp } = useAppStore();
	const { isInitialized: settingsInitialized, initSettings } =
		useSettingsStore();
	const {
		mode,
		resolvedTheme,
		isInitialized: themeInitialized,
		initTheme,
		updateResolvedTheme,
	} = useThemeStore();

	const isLoading =
		!appInitialized || !settingsInitialized || !themeInitialized;

	React.useEffect(() => {
		let cancelled = false;

		const sendReady = () => {
			void emit("frontend-ready").catch(() => { });
		};

		// Emit twice to avoid missing the backend listener during early startup.
		sendReady();
		const timer = window.setTimeout(() => {
			if (!cancelled) {
				sendReady();
			}
		}, 1000);

		return () => {
			cancelled = true;
			window.clearTimeout(timer);
		};
	}, []);

	// Initialize app, settings and theme on mount
	React.useEffect(() => {
		const init = async () => {
			await initApp();
			await initSettings();
			await initTheme();
			await loadCachedPresetModels();
			fetchRemotePresetModels(); // background refresh, no await
		};
		void init();
	}, [initApp, initSettings, initTheme]);

	// Listen for system theme changes
	React.useEffect(() => {
		if (!themeInitialized) return;

		const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

		const handleChange = (e: MediaQueryListEvent) => {
			if (mode !== "system") return;
			updateResolvedTheme(e.matches ? "dark" : "light");
			void refreshTrayMenu().catch(() => { });
		};

		mediaQuery.addEventListener("change", handleChange);
		return () => mediaQuery.removeEventListener("change", handleChange);
	}, [mode, themeInitialized, updateResolvedTheme]);

	// Apply data-theme attribute to document
	React.useEffect(() => {
		if (themeInitialized) {
			document.documentElement.setAttribute("data-theme", resolvedTheme);
		}
	}, [resolvedTheme, themeInitialized]);

	// Set window background color for macOS titlebar
	React.useEffect(() => {
		if (themeInitialized) {
			// Light theme: #ffffff, Dark theme: #1f1f1f
			const bgColor =
				resolvedTheme === "dark"
					? { r: 31, g: 31, b: 31 }
					: { r: 255, g: 255, b: 255 };
			setWindowBackgroundColor(bgColor.r, bgColor.g, bgColor.b).catch(
				console.error,
			);
		}
	}, [resolvedTheme, themeInitialized]);

	// Sync i18n language when app language changes
	React.useEffect(() => {
		if (appInitialized && i18n.language !== language) {
			i18n.changeLanguage(language);
		}
	}, [language, appInitialized]);

	if (isLoading) {
		return (
			<div
				style={{
					display: "flex",
					justifyContent: "center",
					alignItems: "center",
					height: "100vh",
					width: "100vw",
				}}
			>
				<Spin size="large" />
			</div>
		);
	}

	return (
		<ConfigProvider
			locale={antdLocales[language]}
			theme={{
				algorithm:
					resolvedTheme === "dark"
						? antdTheme.darkAlgorithm
						: antdTheme.defaultAlgorithm,
				token: {
					colorPrimary: "#1890ff",
				},
			}}
		>
			<App>
				<AppInitializer>{children}</AppInitializer>
			</App>
		</ConfigProvider>
	);
};
