import { useEffect, useState } from 'react';
import { listDrives, type VolumeSummary } from './api';
import { strings } from './strings';

/** Placeholder shell; the real UI replaces this component. */
export default function App() {
  const [drives, setDrives] = useState<VolumeSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listDrives().then(setDrives, (e) => setError(String(e)));
  }, []);

  return (
    <main>
      <h1>{strings.appName}</h1>
      <h2>{strings.drives}</h2>
      {error && <p>{error}</p>}
      {!error && drives === null && <p>{strings.loadingDrives}</p>}
      {drives && drives.length === 0 && <p>{strings.noDrives}</p>}
      <ul>
        {drives?.map((d) => (
          <li key={d.letter}>
            {d.letter}: {d.label} ({d.fs})
          </li>
        ))}
      </ul>
    </main>
  );
}
