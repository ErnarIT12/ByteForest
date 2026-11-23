import { useAppStore } from '../store';
import { strings } from '../strings';
import { CircleAlert, Icon, X } from './Icon';

/** Dismissible notice for a failed scan or command. */
export function ErrorToast() {
  const error = useAppStore((s) => s.error);
  const title = useAppStore((s) => s.errorTitle);
  const dismiss = useAppStore((s) => s.dismissError);
  if (!error) return null;
  return (
    <div className="toast" role="alert">
      <Icon icon={CircleAlert} size={18} className="toast-icon" />
      <div className="toast-body">
        <strong>{title ?? strings.scanFailed}</strong>
        <span>{error}</span>
      </div>
      <button type="button" className="icon-btn" aria-label={strings.dismiss} onClick={dismiss}>
        <Icon icon={X} />
      </button>
    </div>
  );
}
