import React from 'react';
import { ArrowLeft } from 'lucide-react';

import styles from './styles.module.less';

interface SecondaryPageShellProps {
  title: React.ReactNode;
  subtitle?: React.ReactNode;
  backLabel: string;
  onBack: () => void;
  actions?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  bodyClassName?: string;
}

const joinClassNames = (...classNames: Array<string | undefined | false>) => (
  classNames.filter(Boolean).join(' ')
);

const SecondaryPageShell: React.FC<SecondaryPageShellProps> = ({
  title,
  subtitle,
  backLabel,
  onBack,
  actions,
  children,
  className,
  bodyClassName,
}) => {
  return (
    <div className={joinClassNames(styles.page, className)}>
      <header className={styles.header}>
        <button
          type="button"
          className={styles.backButton}
          onClick={onBack}
          aria-label={backLabel}
          title={backLabel}
        >
          <ArrowLeft size={16} aria-hidden="true" />
        </button>
        <div className={styles.headerTitleBlock}>
          <h1>{title}</h1>
          {subtitle ? <p>{subtitle}</p> : null}
        </div>
        {actions ? <div className={styles.headerActions}>{actions}</div> : null}
      </header>

      <div className={joinClassNames(styles.body, bodyClassName)}>
        {children}
      </div>
    </div>
  );
};

export default SecondaryPageShell;
