import { useAppStore } from '../store';
import { strings } from '../strings';

/** Dismissible notice for a failed scan or command. */
export function ErrorToast() {
  const error = useAppStore((s) => s.error);
  const title = useAppStore((s) => s.errorTitle);
  const dismiss = useAppStore((s) => s.dismissError);
  if (!error) return null;
  return (
    <div className="toast" role="alert">
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="6.5" />
        <path d="M8 4.5v4.2M8 10.8v.4" />
      </svg>
      <div className="toast-body">
        <strong>{title ?? strings.scanFailed}</strong>
        <span>{error}</span>
      </div>
      <button type="button" className="icon-btn" aria-label={strings.dismiss} onClick={dismiss}>
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M4 4l8 8M12 4l-8 8" />
        </svg>
      </button>
    </div>
  );
}
