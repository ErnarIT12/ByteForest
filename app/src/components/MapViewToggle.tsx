import { useAppStore } from '../store';
import { strings } from '../strings';

/** Treemap | Sunburst switch. */
export function MapViewToggle() {
  const view = useAppStore((s) => s.mapView);
  const setView = useAppStore((s) => s.setMapView);
  return (
    <div className="segmented" role="group" aria-label={strings.mapViewLabel}>
      <button type="button" aria-pressed={view === 'treemap'} onClick={() => setView('treemap')}>
        {strings.mapViewTreemap}
      </button>
      <button type="button" aria-pressed={view === 'sunburst'} onClick={() => setView('sunburst')}>
        {strings.mapViewSunburst}
      </button>
    </div>
  );
}
