import React from 'react';
import { joinClassNames } from '../utils/gatewayFormatters';
import styles from './StatTile.module.less';

interface StatTileProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  tone?: 'default' | 'traffic' | 'info' | 'success' | 'warning' | 'error' | 'muted';
  meta?: string;
}

const StatTile: React.FC<StatTileProps> = ({ icon, label, value, tone = 'default', meta }) => (
  <section className={joinClassNames(styles.statTile, styles[`statTile_${tone}`])}>
    <div className={styles.statHeading}>
      <span className={styles.statIcon}>{icon}</span>
      <span className={styles.statLabel}>{label}</span>
    </div>
    <span className={joinClassNames(styles.statValue, styles[`statValue_${tone}`])}>{value}</span>
    {meta ? <span className={styles.statMeta}>{meta}</span> : null}
  </section>
);

export default StatTile;
