import React from 'react';
import { StickyNote } from 'lucide-react';
import ProviderConfigCollapse from './ProviderConfigCollapse';
import styles from './ProviderNotesCollapse.module.less';

interface ProviderNotesCollapseProps {
  title: string;
  value?: string;
  onChange?: React.ChangeEventHandler<HTMLTextAreaElement>;
  placeholder?: string;
  rows?: number;
  resetKey?: string;
  className?: string;
  id?: string;
}

const ProviderNotesCollapse: React.FC<ProviderNotesCollapseProps> = ({
  title,
  value,
  onChange,
  placeholder,
  rows = 3,
  resetKey,
  className,
  id,
}) => {
  const [expanded, setExpanded] = React.useState(false);

  React.useEffect(() => {
    setExpanded(false);
  }, [resetKey]);

  return (
    <ProviderConfigCollapse
      className={className}
      title={title}
      expanded={expanded}
      onExpandedChange={setExpanded}
      icon={<StickyNote />}
    >
      <textarea
        id={id}
        className={styles.textarea}
        rows={rows}
        value={value ?? ''}
        placeholder={placeholder}
        onChange={onChange}
      />
    </ProviderConfigCollapse>
  );
};

export default ProviderNotesCollapse;
