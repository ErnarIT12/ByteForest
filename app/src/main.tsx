import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { initializeAppearance } from './hooks/useColorScheme';
import App from './App';
import './styles.css';
import './controls.css';
import './explorer.css';
import './treemap.css';
import './sidebar.css';
import './overview.css';

initializeAppearance();
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
