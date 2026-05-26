import React from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import styles from './ProviderConfigCollapse.module.less';

interface ProviderConfigCollapseProps {
  title: string;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  children: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
  icon?: React.ReactNode;
}

const ProviderConfigCollapse: React.FC<ProviderConfigCollapseProps> = ({
  title,
  expanded,
  onExpandedChange,
  children,
  actions,
  className,
  icon,
}) => {
  const toggleExpanded = () => onExpandedChange(!expanded);

  const handleHeaderKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.currentTarget !== event.target) {
      return;
    }

    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      toggleExpanded();
    }
  };

  return (
    <div className={[styles.section, className].filter(Boolean).join(' ')}>
      <div
        className={styles.header}
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
        onClick={toggleExpanded}
        onKeyDown={handleHeaderKeyDown}
      >
        <div className={styles.title}>
          {icon && <span className={styles.titleIcon}>{icon}</span>}
          <span>{title}</span>
        </div>
        <div className={styles.headerActions}>
          {actions}
          {expanded ? (
            <ChevronDown className={styles.chevron} aria-hidden="true" />
          ) : (
            <ChevronRight className={styles.chevron} aria-hidden="true" />
          )}
        </div>
      </div>
      <div
        className={`${styles.bodyWrap} ${expanded ? styles.expanded : ''}`}
        hidden={!expanded}
        aria-hidden={!expanded}
      >
        <div className={styles.body}>{children}</div>
      </div>
    </div>
  );
};

export default ProviderConfigCollapse;
