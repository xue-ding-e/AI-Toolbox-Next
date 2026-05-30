import type { CSSProperties, MouseEventHandler, ReactNode } from 'react';
import { Tag } from 'antd';
import styles from './index.module.less';

interface EnabledTagProps {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  onClick?: MouseEventHandler<HTMLSpanElement>;
}

const EnabledTag = ({
  children,
  className,
  style,
  onClick,
}: EnabledTagProps) => {
  const cursor = style?.cursor ?? (onClick ? 'pointer' : 'default');

  return (
    <Tag
      className={[
        'ui-tag',
        'ui-tag-green',
        styles.enabledTag,
        className,
      ].filter(Boolean).join(' ')}
      style={{
        margin: 0,
        cursor,
        ...style,
      }}
      onClick={onClick}
    >
      {children}
    </Tag>
  );
};

export default EnabledTag;
