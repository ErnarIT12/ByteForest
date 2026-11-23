import { Clock, Icon } from './Icon';
import { strings } from '../strings';

/** One row of the Recent Scans list; later milestones add other kinds (such as snapshots). */
export interface RecentItem {
  id: string;
  label: string;
  detail: string;
  /** Tooltip, usually the full path. */
  title: string;
  onSelect(): void;
}

/** Sidebar section listing recent items, one click to open each. */
export function RecentScans({ items }: { items: readonly RecentItem[] }) {
  return (
    <section className="sidebar-section" aria-labelledby="sidebar-recent">
      <h2 id="sidebar-recent" className="sidebar-heading">{strings.recentTitle}</h2>
      {items.length === 0 ? (
        <p className="sidebar-note">{strings.recentEmpty}</p>
      ) : (
        <ul className="sidebar-list">
          {items.map((item) => (
            <li key={item.id}>
              <button type="button" className="sidebar-item" title={item.title}
                aria-label={strings.recentRescan(item.label)} onClick={item.onSelect}>
                <Icon icon={Clock} className="sidebar-icon" />
                <span className="sidebar-text">
                  <span className="sidebar-name">{item.label}</span>
                  <span className="sidebar-detail">{item.detail}</span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
