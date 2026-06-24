import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'

// Global error handler — prevent white-screen crashes
window.onerror = (msg, src, line, col, err) => {
  document.body.innerHTML = `<div style="padding:2rem;font-family:sans-serif;color:#ef4444;background:#1a1a1a;min-height:100vh">
    <h2>App Error</h2>
    <pre style="white-space:pre-wrap;font-size:13px">${msg}\n${src}:${line}:${col}\n${err?.stack || ''}</pre>
  </div>`;
  return false;
};
window.onunhandledrejection = (e) => {
  document.body.innerHTML = `<div style="padding:2rem;font-family:sans-serif;color:#ef4444;background:#1a1a1a;min-height:100vh">
    <h2>Unhandled Promise Rejection</h2>
    <pre style="white-space:pre-wrap;font-size:13px">${String(e.reason)}</pre>
  </div>`;
};

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
