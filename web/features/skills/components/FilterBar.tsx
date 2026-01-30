import React from 'react';
import { Input, Select } from 'antd';
import { SearchOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useSkillsStore } from '../stores/skillsStore';
import styles from './FilterBar.module.less';

export const FilterBar: React.FC = () => {
  const { t } = useTranslation();
  const { searchQuery, setSearchQuery, sortMode, setSortMode } = useSkillsStore();

  return (
    <div className={styles.filterBar}>
      <Input
        placeholder={t('skills.searchPlaceholder')}
        prefix={<SearchOutlined />}
        value={searchQuery}
        onChange={(e) => setSearchQuery(e.target.value)}
        allowClear
        className={styles.searchInput}
      />
      <Select
        value={sortMode}
        onChange={setSortMode}
        options={[
          { value: 'updated', label: t('skills.sortByUpdated') },
          { value: 'name', label: t('skills.sortByName') },
        ]}
        className={styles.sortSelect}
      />
    </div>
  );
};
