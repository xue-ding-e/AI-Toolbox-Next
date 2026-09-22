import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './app/App';
import './app/monaco';
import { installEditableContextMenuFocus } from '@/utils/contextMenuFocus';
import { installEmptyPasteFallback } from '@/utils/emptyPasteFallback';
import '@/i18n';
import './App.css';

installEditableContextMenuFocus();
installEmptyPasteFallback();

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
