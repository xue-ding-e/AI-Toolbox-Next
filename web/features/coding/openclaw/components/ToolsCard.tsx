import React from 'react';
import { Button, Form, Input, Select, Tag, Typography, message } from 'antd';
import { PlusOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { setOpenClawTools } from '@/services/openclawApi';
import type { OpenClawToolsConfig } from '@/types/openclaw';

const { Text } = Typography;

const PROFILE_OPTIONS = [
  { value: 'default', labelKey: 'openclaw.tools.profileDefault' },
  { value: 'strict', labelKey: 'openclaw.tools.profileStrict' },
  { value: 'permissive', labelKey: 'openclaw.tools.profilePermissive' },
  { value: 'custom', labelKey: 'openclaw.tools.profileCustom' },
];

interface Props {
  tools: OpenClawToolsConfig | null;
  onSaved: () => void;
}

const formItemLayout = {
  labelCol: { span: 6 },
  wrapperCol: { span: 18 },
};

const ToolsCard: React.FC<Props> = ({ tools, onSaved }) => {
  const { t } = useTranslation();
  const [saving, setSaving] = React.useState(false);
  const [profile, setProfile] = React.useState('default');
  const [allowList, setAllowList] = React.useState<string[]>([]);
  const [denyList, setDenyList] = React.useState<string[]>([]);
  const [allowInput, setAllowInput] = React.useState('');
  const [denyInput, setDenyInput] = React.useState('');

  React.useEffect(() => {
    if (tools) {
      setProfile(tools.profile || 'default');
      setAllowList(tools.allow || []);
      setDenyList(tools.deny || []);
    }
  }, [tools]);

  const handleAddAllow = () => {
    const value = allowInput.trim();
    if (value && !allowList.includes(value)) {
      setAllowList([...allowList, value]);
      setAllowInput('');
    }
  };

  const handleRemoveAllow = (item: string) => {
    setAllowList(allowList.filter((a) => a !== item));
  };

  const handleAddDeny = () => {
    const value = denyInput.trim();
    if (value && !denyList.includes(value)) {
      setDenyList([...denyList, value]);
      setDenyInput('');
    }
  };

  const handleRemoveDeny = (item: string) => {
    setDenyList(denyList.filter((d) => d !== item));
  };

  const handleSave = async () => {
    try {
      setSaving(true);
      const toolsConfig: OpenClawToolsConfig = {
        ...(tools || {}),
        profile,
        allow: allowList,
        deny: denyList,
      };
      await setOpenClawTools(toolsConfig);
      message.success(t('common.success'));
      onSaved();
    } catch (error) {
      console.error('Failed to save tools config:', error);
      message.error(t('common.error'));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Form layout="horizontal" {...formItemLayout} style={{ maxWidth: 600 }}>
      {/* Profile */}
      <Form.Item label={<Text strong>{t('openclaw.tools.profile')}</Text>}>
        <Select
          value={profile}
          onChange={setProfile}
          options={PROFILE_OPTIONS.map((opt) => ({
            value: opt.value,
            label: t(opt.labelKey),
          }))}
        />
      </Form.Item>

      {/* Allow list */}
      <Form.Item label={<Text strong>{t('openclaw.tools.allowList')}</Text>}>
        <div>
          <div style={{ marginBottom: allowList.length > 0 ? 8 : 0 }}>
            {allowList.map((item) => (
              <Tag key={item} closable onClose={() => handleRemoveAllow(item)} color="green" style={{ marginBottom: 4 }}>
                {item}
              </Tag>
            ))}
          </div>
          <Input
            size="small"
            value={allowInput}
            onChange={(e) => setAllowInput(e.target.value)}
            onPressEnter={handleAddAllow}
            placeholder={t('openclaw.tools.addPattern')}
            suffix={
              <Button type="text" size="small" icon={<PlusOutlined />} onClick={handleAddAllow} style={{ marginRight: -4 }} />
            }
          />
        </div>
      </Form.Item>

      {/* Deny list */}
      <Form.Item label={<Text strong>{t('openclaw.tools.denyList')}</Text>}>
        <div>
          <div style={{ marginBottom: denyList.length > 0 ? 8 : 0 }}>
            {denyList.map((item) => (
              <Tag key={item} closable onClose={() => handleRemoveDeny(item)} color="red" style={{ marginBottom: 4 }}>
                {item}
              </Tag>
            ))}
          </div>
          <Input
            size="small"
            value={denyInput}
            onChange={(e) => setDenyInput(e.target.value)}
            onPressEnter={handleAddDeny}
            placeholder={t('openclaw.tools.addPattern')}
            suffix={
              <Button type="text" size="small" icon={<PlusOutlined />} onClick={handleAddDeny} style={{ marginRight: -4 }} />
            }
          />
        </div>
      </Form.Item>

      <Form.Item wrapperCol={{ offset: 6, span: 18 }}>
        <div style={{ textAlign: 'right' }}>
          <Button type="primary" onClick={handleSave} loading={saving}>
            {t('openclaw.tools.save')}
          </Button>
        </div>
      </Form.Item>
    </Form>
  );
};

export default ToolsCard;
