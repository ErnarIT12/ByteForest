import { useAppStore } from '../store';
import { strings } from '../strings';

/** Path from the scan root to the focused folder; each ancestor zooms out on click. */
export function Breadcrumbs() {
  const path = useAppStore((s) => s.focusPath);
  const setFocus = useAppStore((s) => s.setFocus);
  const goUp = useAppStore((s) => s.goUp);

  return (
    <nav className="breadcrumbs" aria-label={strings.breadcrumbsLabel}>
      <button
        type="button"
        className="icon-btn"
        aria-label={strings.up}
        title={strings.up}
        disabled={path.length <= 1}
        onClick={goUp}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 13V3M3.5 7.5L8 3l4.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      <ol>
        {path.map((c, i) => {
          const last = i === path.length - 1;
          return (
            <li key={c.id}>
              {i > 0 && (
                <svg className="crumb-sep" viewBox="0 0 16 16" aria-hidden="true">
                  <path d="M6 4l4 4-4 4" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              )}
              {last ? (
                <span className="crumb current" aria-current="location" title={c.name}>
                  {c.name}
                </span>
              ) : (
                <button type="button" className="crumb" title={c.name} onClick={() => setFocus(c.id)}>
                  {c.name}
                </button>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
