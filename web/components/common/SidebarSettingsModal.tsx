import React from 'react';
import { Modal, Switch } from 'antd';
import { useTranslation } from 'react-i18next';

interface SidebarSettingsModalProps {
  open: boolean;
  onClose: () => void;
  sidebarVisible: boolean;
  onSidebarVisibleChange: (visible: boolean) => void | Promise<void>;
}

const SidebarSettingsModal: React.FC<SidebarSettingsModalProps> = ({
  open,
  onClose,
  sidebarVisible,
  onSidebarVisibleChange,
}) => {
  const { t } = useTranslation();

  return (
    <Modal
      title={t('common.moreOptions')}
      open={open}
      onCancel={onClose}
      footer={null}
      width={520}
    >
      <div style={{ display: 'flex', alignItems: 'flex-start', gap: 16 }}>
        <div style={{ width: 180, paddingTop: 4, color: 'var(--color-text-primary)', fontWeight: 500 }}>
          {t('common.showSidebar')}
        </div>
        <div style={{ flex: 1 }}>
          <Switch checked={sidebarVisible} onChange={onSidebarVisibleChange} />
        </div>
      </div>
    </Modal>
  );
};

export default SidebarSettingsModal;
