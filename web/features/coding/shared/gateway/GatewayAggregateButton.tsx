import React from 'react';
import { Layers, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import chipStyles from './gatewayStatusChip.module.less';

interface GatewayAggregateButtonProps {
  /** Marks the chip as the mode the CLI currently runs in (primary tint). */
  current?: boolean;
  /** Keeps the chip in place with a spinner while the settings drawer loads. */
  loading?: boolean;
  onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void;
}

const joinClassNames = (...classNames: Array<string | false | null | undefined>) =>
  classNames.filter(Boolean).join(' ');

/**
 * Entry chip for the gateway aggregate settings, shown next to the takeover
 * status chip in the provider list header.  It shares
 * `gatewayStatusChip.module.less` with `GatewayFailoverButton` so both chips
 * keep the same pill geometry, typography and hover behaviour.
 */
const GatewayAggregateButton: React.FC<GatewayAggregateButtonProps> = ({
  current = false,
  loading = false,
  onClick,
}) => {
  const { t } = useTranslation();

  return (
    <span className={chipStyles.shell} onClick={(event) => event.stopPropagation()}>
      <button
        type="button"
        className={joinClassNames(chipStyles.chip, current && chipStyles.chipCurrent)}
        title={t('gateway.aggregate.buttonTooltip')}
        aria-busy={loading || undefined}
        disabled={loading}
        onClick={onClick}
      >
        {loading ? (
          <Loader2 size={12} className={chipStyles.spin} aria-hidden="true" />
        ) : (
          <Layers size={12} className={chipStyles.icon} aria-hidden="true" />
        )}
        <span>{t('gateway.aggregate.button')}</span>
      </button>
    </span>
  );
};

export default GatewayAggregateButton;
