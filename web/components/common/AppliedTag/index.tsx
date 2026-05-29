import type { CSSProperties, MouseEventHandler, ReactNode } from 'react';
import { Tag } from 'antd';
import { CircleCheckBig } from 'lucide-react';
import styles from './index.module.less';

interface AppliedTagProps {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  onClick?: MouseEventHandler<HTMLSpanElement>;
}

const AppliedTag = ({
  children,
  className,
  style,
  onClick,
}: AppliedTagProps) => {
  const cursor = style?.cursor ?? (onClick ? 'pointer' : 'default');

  return (
    <Tag
      data-state="closed"
      className={[
        'ui-tag',
        'ui-tag-green',
        styles.appliedTag,
        className,
      ].filter(Boolean).join(' ')}
      style={{
        margin: 0,
        cursor,
        ...style,
      }}
      onClick={onClick}
    >
      <span
        role="img"
        aria-label="circle-check-big"
        className={`anticon anticon-circle-check-big ${styles.appliedTagIcon}`}
      >
        <CircleCheckBig
          aria-hidden="true"
          focusable="false"
          style={{
            display: 'inline-block',
            width: '1em',
            height: '1em',
            flexShrink: 0,
          }}
        />
      </span>
      {children}
    </Tag>
  );
};

export default AppliedTag;
