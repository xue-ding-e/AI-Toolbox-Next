import React from 'react';
import { Button, Input } from 'antd';
import { useTranslation } from 'react-i18next';
import { SearchOutlined } from '@ant-design/icons';

interface ProviderSearchInputProps {
  value: string;
  onChange: (keyword: string) => void;
}

/**
 * Collapsible provider search box for the provider section header (left of
 * the sort button). Collapsed by default as an icon+label link button that
 * matches the other header buttons; expands into a fixed-width input on
 * click. Clicks must not propagate to the antd Collapse header or they would
 * toggle the section.
 *
 * Collapse rules: pressing Escape clears and collapses; blurring with an
 * empty value collapses; blurring while a keyword is active keeps the input
 * visible so the filtered result stays explainable.
 */
const ProviderSearchInput: React.FC<ProviderSearchInputProps> = ({ value, onChange }) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = React.useState(false);

  const collapse = () => {
    setExpanded(false);
    onChange('');
  };

  if (!expanded) {
    return (
      <Button
        type="link"
        size="small"
        style={{ fontSize: 12 }}
        icon={<SearchOutlined />}
        onClick={(event) => {
          event.stopPropagation();
          setExpanded(true);
        }}
      >
        {t('common.search')}
      </Button>
    );
  }

  return (
    <Input
      size="small"
      autoFocus
      allowClear
      value={value}
      onChange={(event) => onChange(event.target.value)}
      onClick={(event) => event.stopPropagation()}
      onBlur={() => {
        if (!value.trim()) {
          setExpanded(false);
        }
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.stopPropagation();
          collapse();
        }
      }}
      placeholder={t('common.providerSearch.placeholder')}
      prefix={<SearchOutlined style={{ color: 'var(--color-text-tertiary)' }} />}
      style={{ width: 160 }}
    />
  );
};

export default ProviderSearchInput;
