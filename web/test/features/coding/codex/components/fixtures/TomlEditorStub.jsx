import React from 'react';

// Keep these checks focused on form state and model import, independent of workers.
export default function TomlEditorStub({ value = '', onChange }) {
  return <textarea aria-label="Fixture TOML editor" value={value} onChange={event => onChange?.(event.target.value)}
    style={{ width: '100%', height: 220, color: 'var(--color-text-primary)', background: 'var(--color-bg-container)' }} />;
}
