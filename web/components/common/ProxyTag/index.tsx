import type { CSSProperties, MouseEventHandler, ReactNode } from 'react';
import { Tag } from 'antd';
import { Network } from 'lucide-react';
import styles from './index.module.less';

interface ProxyTagProps {
  children: ReactNode;
  className?: string;
  style?: CSSProperties;
  onClick?: MouseEventHandler<HTMLSpanElement>;
}

const ProxyTag = ({
  children,
  className,
  style,
  onClick,
}: ProxyTagProps) => {
  const cursor = style?.cursor ?? (onClick ? 'pointer' : 'default');

  return (
    <Tag
      className={[
        'ui-tag',
        'ui-tag-green',
        styles.proxyTag,
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
        aria-label="gateway"
        className={`anticon anticon-network ${styles.proxyTagIcon}`}
      >
        <Network
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

export default ProxyTag;
