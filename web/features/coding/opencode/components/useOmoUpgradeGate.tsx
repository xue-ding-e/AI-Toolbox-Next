import React from 'react';
import { Button, Modal, Typography, message } from 'antd';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '@/stores';

type OmoUpgradeChoice = 'upgraded' | 'legacy';

/**
 * One-time gate shown on the first Oh My OpenAgent apply action.
 *
 * The unified (`~/.omo/omo.jsonc`, `[opencode]` block) and legacy
 * (`~/.config/opencode/oh-my-openagent.jsonc`) config formats are incompatible, and
 * only the newest oh-my-openagent reads the unified one. Before the first apply we ask
 * whether omo has been upgraded to the latest version:
 *  - upgraded → ensure the legacy write toggle is off (unified mode), then apply
 *  - legacy   → auto-enable the "write legacy config" toggle, then apply, and tell the
 *               user they can turn that toggle off again in "more options" after upgrading
 *  - cancel   → abort; the flag is NOT persisted, so the dialog reappears on the next apply
 * Once a definitive choice is persisted the dialog never shows again.
 */
export function useOmoUpgradeGate() {
	const { t } = useTranslation();
	const {
		opencodeOmoUpgradeConfirmed,
		setOpencodeOmoUpgradeConfirmed,
		opencodeUseLegacyOhMyConfig,
		setOpencodeUseLegacyOhMyConfig,
	} = useSettingsStore();
	const [open, setOpen] = React.useState(false);
	const pendingApplyRef = React.useRef<(() => Promise<void>) | null>(null);

	const guardedApply = React.useCallback(
		(apply: () => Promise<void>) => {
			if (opencodeOmoUpgradeConfirmed) {
				void apply();
				return;
			}
			pendingApplyRef.current = apply;
			setOpen(true);
		},
		[opencodeOmoUpgradeConfirmed],
	);

	const handleChoice = React.useCallback(
		async (choice: OmoUpgradeChoice) => {
			setOpen(false);
			const apply = pendingApplyRef.current;
			pendingApplyRef.current = null;
			await setOpencodeOmoUpgradeConfirmed(true);
			if (choice === 'legacy') {
				await setOpencodeUseLegacyOhMyConfig(true);
				message.info(t('opencode.ohMyOpenCode.upgradeLegacyAutoHint'));
			} else if (opencodeUseLegacyOhMyConfig) {
				// 已升级：新版 omo 读 unified 格式，若用户此前手动开过 legacy 开关则关闭并明确提示
				await setOpencodeUseLegacyOhMyConfig(false);
				message.info(t('opencode.ohMyOpenCode.upgradeUnifiedAutoHint'));
			}
			if (apply) void apply();
		},
		[
			setOpencodeOmoUpgradeConfirmed,
			opencodeUseLegacyOhMyConfig,
			setOpencodeUseLegacyOhMyConfig,
			t,
		],
	);

	const handleCancel = React.useCallback(() => {
		setOpen(false);
		pendingApplyRef.current = null;
	}, []);

	const upgradeConfirmModal = React.useMemo(
		() => (
			<Modal
				open={open}
				title={t('opencode.ohMyOpenCode.upgradeConfirmTitle')}
				onCancel={handleCancel}
				maskClosable={false}
				footer={[
					<Button key="cancel" onClick={handleCancel}>
						{t('common.cancel')}
					</Button>,
					<Button
						key="legacy"
						onClick={() => {
							void handleChoice('legacy');
						}}
					>
						{t('opencode.ohMyOpenCode.upgradeConfirmNotUpgraded')}
					</Button>,
					<Button
						key="upgraded"
						type="primary"
						onClick={() => {
							void handleChoice('upgraded');
						}}
					>
						{t('opencode.ohMyOpenCode.upgradeConfirmUpgraded')}
					</Button>,
				]}
			>
				<Typography.Text type="secondary">
					{t('opencode.ohMyOpenCode.upgradeConfirmDesc')}
				</Typography.Text>
			</Modal>
		),
		[open, t, handleCancel, handleChoice],
	);

	return { guardedApply, upgradeConfirmModal };
}
