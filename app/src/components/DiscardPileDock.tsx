import { strings } from '../strings';
import { Icon, Trash2 } from './Icon';

/** Bottom dock of the sidebar for the Discard Pile (items can be queued here in a later update). */
export function DiscardPileDock() {
  return (
    <section className="discard-dock" aria-labelledby="sidebar-discard" title={strings.discardEmpty}>
      <h2 id="sidebar-discard" className="sidebar-heading discard-heading">
        <Icon icon={Trash2} className="sidebar-icon" />
        <span className="sidebar-text">{strings.discardTitle}</span>
      </h2>
      <p className="sidebar-note">{strings.discardEmpty}</p>
    </section>
  );
}
