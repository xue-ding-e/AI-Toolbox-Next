import React from 'react';
import { Card } from 'antd';
import { HolderOutlined } from '@ant-design/icons';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { useTranslation } from 'react-i18next';
import { ManagementCheckbox } from '@/features/coding/shared/management';

interface CardShellProps {
  /** Sortable id; without one the handle is omitted and the card is pinned. */
  sortableId?: string;
  draggable?: boolean;
  /** Renders a checkbox instead of the drag handle. */
  selectable?: boolean;
  selected?: boolean;
  onSelectChange?: (checked: boolean) => void;
  /** Renders the whole card at reduced opacity. */
  dimmed?: boolean;
  /** The card body. */
  children: React.ReactNode;
}

/**
 * The frame every provider card style shares: drag registration, the handle or
 * selection checkbox, and the card chrome.
 *
 * Kept as one component because these three are the parts that must stay
 * identical across styles — a card that forgets `setNodeRef` cannot be dragged,
 * and one that reimplements the handle drifts in cursor and hit area. The
 * *content* layout is what the three styles differ on, and that is left to
 * them.
 */
const CardShell: React.FC<CardShellProps> = ({
  sortableId,
  draggable = false,
  selectable = false,
  selected = false,
  onSelectChange,
  dimmed = false,
  children,
}) => {
  const { t } = useTranslation();

  // `useSortable` must run on every render, so a card rendered without an id
  // still registers — under a placeholder, disabled.
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: sortableId ?? 'provider-card',
    disabled: !draggable,
  });

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : dimmed ? 0.6 : 1,
  };

  const showHandle = draggable && !selectable;

  // The ref is attached even when dragging is off: dnd-kit measures the node to
  // compute transforms, and an unregistered node cannot be dragged the moment
  // the parent re-enables it.
  return (
    <div ref={setNodeRef} style={style}>
      {/* Bottom margin on the Card, not the wrapper: each card spaces itself so
          the gap survives a reorder among its siblings. */}
      <Card
        style={{
          marginBottom: 12,
          borderColor: selectable && selected ? 'var(--ant-color-primary)' : 'var(--color-border-card)',
          boxShadow: 'var(--shadow-card-sm)',
          transition: 'box-shadow 0.16s ease',
        }}
        styles={{ body: { padding: '8px 12px' } }}
        onMouseEnter={(event) => {
          event.currentTarget.style.boxShadow = 'var(--shadow-card-sm-hover)';
        }}
        onMouseLeave={(event) => {
          event.currentTarget.style.boxShadow = 'var(--shadow-card-sm)';
        }}
      >
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12 }}>
          {selectable ? (
            <div style={{ display: 'flex', alignItems: 'center', padding: '4px 0' }}>
              <ManagementCheckbox
                checked={selected}
                ariaLabel={t('common.batch.selectItem')}
                onChange={onSelectChange ?? (() => {})}
              />
            </div>
          ) : showHandle ? (
            <div
              {...attributes}
              {...listeners}
              style={{
                cursor: isDragging ? 'grabbing' : 'grab',
                padding: '4px 0',
                display: 'flex',
                alignItems: 'center',
                color: '#999',
                touchAction: 'none',
              }}
            >
              <HolderOutlined style={{ fontSize: 16 }} />
            </div>
          ) : null}

          <div style={{ flex: 1, minWidth: 0 }}>{children}</div>
        </div>
      </Card>
    </div>
  );
};

export default CardShell;
