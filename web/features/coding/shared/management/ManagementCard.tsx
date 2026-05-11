import React from 'react';
import { GripVertical } from 'lucide-react';
import styles from './ManagementCard.module.less';

interface ManagementCardProps {
  containerRef?: (node: HTMLDivElement | null) => void;
  containerStyle?: React.CSSProperties;
  selected?: boolean;
  selectable?: boolean;
  children: React.ReactNode;
  className?: string;
}

export const ManagementCard: React.FC<ManagementCardProps> = ({
  containerRef,
  containerStyle,
  selected,
  selectable,
  children,
  className,
}) => (
  <div ref={containerRef} style={containerStyle} className={styles.cardContainer}>
    <div className={`${styles.card}${selectable && selected ? ` ${styles.selected}` : ''}${className ? ` ${className}` : ''}`}>
      {children}
    </div>
  </div>
);

export const ManagementCardCheckboxArea: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className={styles.checkboxArea}>{children}</div>
);

interface ManagementCardDragHandleProps extends React.HTMLAttributes<HTMLDivElement> {
  listeners?: Record<string, Function>;
}

export const ManagementCardDragHandle: React.FC<ManagementCardDragHandleProps> = ({ listeners, ...props }) => (
  <div className={styles.dragHandle} {...props} {...listeners}>
    <GripVertical size={15} aria-hidden="true" />
  </div>
);

interface ManagementCardIconProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  icon: React.ReactNode;
  asButton?: boolean;
}

export const ManagementCardIcon: React.FC<ManagementCardIconProps> = ({ icon, asButton, className, ...props }) => {
  if (asButton || props.onClick) {
    return (
      <button
        {...props}
        type={props.type ?? 'button'}
        className={`${styles.iconArea} ${styles.clickableIconArea}${className ? ` ${className}` : ''}`}
      >
        {icon}
      </button>
    );
  }
  return (
    <div className={`${styles.iconArea}${className ? ` ${className}` : ''}`} title={props.title}>
      {icon}
    </div>
  );
};

export const ManagementCardMain: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className={styles.main}>{children}</div>
);

export const ManagementCardHeader: React.FC<{ title: React.ReactNode; meta?: React.ReactNode; minWidth?: number }> = ({ title, meta, minWidth = 120 }) => (
  <div className={styles.headerRow} style={{ '--card-header-min': `${minWidth}px` } as React.CSSProperties}>
    <div className={styles.name}>{title}</div>
    {meta && <div className={styles.headerMeta}>{meta}</div>}
  </div>
);

export const ManagementCardMetaRow: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className={styles.metaRow}>{children}</div>
);

export const ManagementCardToolMatrix: React.FC<{ children: React.ReactNode; className?: string }> = ({ children, className }) => (
  <div className={`${styles.toolMatrix}${className ? ` ${className}` : ''}`}>{children}</div>
);

export const ManagementCardActions: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className={styles.actions}>{children}</div>
);
