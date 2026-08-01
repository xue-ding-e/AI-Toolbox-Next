import React from 'react';
import { joinClassNames } from '../utils/gatewayFormatters';
import styles from './StatTile.module.less';

interface StatTileProps {
  icon: React.ReactNode;
  label: string;
  value: string;
  tone?: 'default' | 'traffic' | 'info' | 'success' | 'warning' | 'error' | 'muted';
  meta?: string;
  visual?: StatVisualKind;
}

type StatVisualKind = 'curve' | 'stack' | 'coins';

const StatVisualGraphic: React.FC<{ visual: StatVisualKind }> = ({ visual }) => {
  if (visual === 'curve') {
    return (
      <svg
        className={styles.visualSvg}
        viewBox="0 0 190 120"
        preserveAspectRatio="none"
        focusable="false"
      >
        <path
          className={styles.visualWave}
          d="M0 118 C18 80 30 54 56 52 C82 50 88 88 115 82 C143 76 154 40 190 38"
        />
      </svg>
    );
  }

  if (visual === 'stack') {
    return (
      <svg
        className={styles.visualSvg}
        viewBox="0 0 150 160"
        preserveAspectRatio="xMidYMax meet"
        focusable="false"
      >
        <g transform="translate(10 -6)" opacity="0.72">
          <polygon className={styles.visualCubeTop} points="70,0 140,38 70,76 0,38" />
          <polygon className={styles.visualCubeLeft} points="0,38 70,76 70,148 0,110" />
          <polygon className={styles.visualCubeRight} points="70,76 140,38 140,110 70,148" />
        </g>
        <g transform="translate(2 30)" opacity="0.58">
          <polygon className={styles.visualCubeTop} points="70,0 140,38 70,76 0,38" />
          <polygon className={styles.visualCubeLeft} points="0,38 70,76 70,148 0,110" />
          <polygon className={styles.visualCubeRight} points="70,76 140,38 140,110 70,148" />
        </g>
        <g transform="translate(-6 66)" opacity="0.42">
          <polygon className={styles.visualCubeTop} points="70,0 140,38 70,76 0,38" />
          <polygon className={styles.visualCubeLeft} points="0,38 70,76 70,148 0,110" />
          <polygon className={styles.visualCubeRight} points="70,76 140,38 140,110 70,148" />
        </g>
      </svg>
    );
  }

  return (
    <svg
      className={styles.visualSvg}
      viewBox="0 0 160 160"
      preserveAspectRatio="xMidYMax meet"
      focusable="false"
    >
      <g transform="translate(26 0)">
        <g transform="rotate(-16 108 100)">
          <path
            className={styles.visualCoinEdge}
            d="M66 106c0-25 19-46 42-46s42 21 42 46c0 6-2 12-5 17-7 13-20 22-37 22s-30-9-37-22c-3-5-5-11-5-17Z"
          />
          <circle className={styles.visualCoinFace} cx="108" cy="98" r="44" />
          <path
            className={styles.visualCoinMark}
            d="M122 86c-4-5-9-7-14-7-6 0-12 3-12 8 0 11 24 8 24 19 0 6-6 10-12 10-5 0-10-2-14-7M108 72v52"
          />
          <path className={styles.visualCoinShine} d="M84 78c6-9 16-14 28-15" />
        </g>
        <g transform="rotate(-12 66 128)">
          <circle className={styles.visualCoinSmallFace} cx="66" cy="128" r="24" />
          <path
            className={styles.visualCoinMark}
            d="M73 122c-2-2-4-3-7-3-3 0-6 1-6 4 0 6 13 4 13 10 0 3-3 5-7 5-3 0-5-1-7-3M66 116v24"
          />
        </g>
      </g>
    </svg>
  );
};

const StatTile: React.FC<StatTileProps> = ({
  icon,
  label,
  value,
  tone = 'default',
  meta,
  visual,
}) => (
  <section className={joinClassNames(styles.statTile, styles[`statTile_${tone}`])}>
    <div className={styles.statHeading}>
      <span className={styles.statIcon}>{icon}</span>
      <span className={styles.statLabel}>{label}</span>
    </div>
    <span className={joinClassNames(styles.statValue, styles[`statValue_${tone}`])}>{value}</span>
    {meta ? <span className={styles.statMeta}>{meta}</span> : null}
    {visual ? (
      <span
        className={joinClassNames(styles.statVisual, styles[`statVisual_${visual}`])}
        aria-hidden="true"
      >
        <StatVisualGraphic visual={visual} />
      </span>
    ) : null}
  </section>
);

export default StatTile;
