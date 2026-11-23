import { useAppStore } from '../store';
import { strings } from '../strings';
import { ArrowUp, ChevronRight, Icon } from './Icon';

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
        <Icon icon={ArrowUp} />
      </button>
      <ol>
        {path.map((c, i) => {
          const last = i === path.length - 1;
          return (
            <li key={c.id}>
              {i > 0 && (
                <Icon icon={ChevronRight} size={14} className="crumb-sep" />
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
