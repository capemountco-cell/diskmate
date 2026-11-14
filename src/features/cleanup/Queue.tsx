import { useEffect } from "react";

import { Button, Note, Panel } from "../../components/primitives";
import { formatBytes, formatCount, truncatePath } from "../../lib/format";

import type { CleanupController } from "./useCleanup";

/**
 * The bar that follows you through Cleanup, Duplicates and Applications.
 *
 * Adding something to the queue in one section and finding it gone in the next
 * would make the queue feel like three queues. It is one, so its summary sits
 * at the bottom of every section that can add to it, and opening it shows the
 * one place anything is ever removed.
 */
export function QueueBar({
  cleanup,
  onOpen,
}: {
  cleanup: CleanupController;
  onOpen: () => void;
}) {
  if (cleanup.staged.length === 0) return null;
  return (
    <div className="queue-bar" role="region" aria-label="Queue">
      <span className="queue-bar__text">
        <b className="num">{formatCount(cleanup.staged.length)}</b>{" "}
        {cleanup.staged.length === 1 ? "item" : "items"} in your queue ·{" "}
        <b className="num">{formatBytes(cleanup.stagedTotal)}</b>
      </span>
      <span className="queue-bar__actions">
        <button
          type="button"
          className="link-button"
          onClick={() => cleanup.unstage(cleanup.staged.map((c) => c.id))}
        >
          Clear
        </button>
        <Button variant="primary" onClick={onOpen}>
          Review and move to Trash
        </Button>
      </span>
    </div>
  );
}

/** The queue, opened over the current section. */
export function QueueDrawer({
  cleanup,
  open,
  onClose,
}: {
  cleanup: CleanupController;
  open: boolean;
  onClose: () => void;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="drawer" role="dialog" aria-modal="true" aria-label="Queue">
      <button type="button" className="drawer__scrim" aria-label="Close" onClick={onClose} />
      <div className="drawer__panel">
        <div className="row row--between" style={{ marginBottom: "var(--space-3)" }}>
          <strong style={{ fontSize: "var(--text-md)" }}>Queue</strong>
          <Button variant="quiet" onClick={onClose}>
            Close
          </Button>
        </div>
        <Queue cleanup={cleanup} />
      </div>
    </div>
  );
}

/** What happened the last time the queue was approved. */
export function RemovalReportNote({ cleanup }: { cleanup: CleanupController }) {
  if (!cleanup.report) return null;
  return (
    <Note
      tone={cleanup.report.failedCount > 0 ? "caution" : "info"}
      title={`Moved ${formatCount(cleanup.report.removedCount)} items to the Trash, freeing ${formatBytes(cleanup.report.removedBytes)}.`}
    >
      {cleanup.report.refusedCount > 0 && `${cleanup.report.refusedCount} were refused. `}
      {cleanup.report.failedCount > 0 && `${cleanup.report.failedCount} could not be moved. `}
      Everything removed is still in your Trash until you empty it.{" "}
      <button type="button" className="button button--quiet" onClick={cleanup.dismissReport}>
        Dismiss
      </button>
    </Note>
  );
}

/**
 * The queue.
 *
 * The only place anything is removed, and the reason the rest of the app can
 * be browsed without consequence.
 */
function Queue({ cleanup }: { cleanup: CleanupController }) {
  const empty = cleanup.staged.length === 0;

  return (
    <Panel
      title="Ready to move"
      note={empty ? "Nothing selected yet." : "Review this, then remove it."}
    >
      <div className="queue__total">{formatBytes(cleanup.stagedTotal)}</div>
      <div className="stat__hint">
        {empty
          ? "Add things from Cleanup, Duplicates or Applications and they appear here."
          : `${formatCount(cleanup.staged.length)} items ready to move to the Trash`}
      </div>

      {!empty && (
        <>
          <div className="queue__list">
            {cleanup.staged.map((candidate) => (
              <div className="queue__item" key={candidate.id}>
                <span className="queue__item-name">{candidate.name}</span>
                <span className="candidate__size">{formatBytes(candidate.bytes)}</span>
                <span className="queue__item-path mono" title={candidate.path}>
                  {truncatePath(candidate.path, 46)}
                </span>
              </div>
            ))}
          </div>

          <div
            className="row"
            style={{ marginTop: "var(--space-4)", justifyContent: "space-between" }}
          >
            <Button
              variant="quiet"
              onClick={() => cleanup.unstage(cleanup.staged.map((c) => c.id))}
            >
              Empty queue
            </Button>
            <Button variant="primary" onClick={cleanup.removeApproved} disabled={cleanup.busy}>
              Move to Trash
            </Button>
          </div>

          <RemovalFlow />

          <p className="stat__hint" style={{ marginTop: "var(--space-3)" }}>
            Mr Clean removes exactly the {formatCount(cleanup.staged.length)} items listed here,
            totalling {formatBytes(cleanup.stagedTotal)}, and nothing else. If the list or the
            total changes before you click, the whole operation is refused rather than partly
            done.
          </p>
        </>
      )}
    </Panel>
  );
}

/**
 * What approving the queue does, as the steps it does them in.
 *
 * This replaced a sentence promising the removal was safe. A promise is what
 * every cleaner on the App Store makes; the steps are checkable.
 */
function RemovalFlow() {
  const steps = [
    { key: "queue", title: "Your list", note: "only what you added", icon: ListIcon },
    { key: "check", title: "Checked", note: "protected, changed, gone", icon: ShieldIcon },
    { key: "trash", title: "Trash", note: "not deleted", icon: TrashIcon },
    { key: "back", title: "Put Back", note: "whenever you like", icon: UndoIcon },
  ];

  return (
    <ol className="flow">
      {steps.map((step, index) => (
        <li key={step.key} className="flow__step">
          {index > 0 && (
            <svg className="flow__arrow" viewBox="0 0 16 16" aria-hidden="true">
              <path d="M3.5 8h8M8 4.5L11.5 8 8 11.5" />
            </svg>
          )}
          <span className="flow__icon">
            <step.icon />
          </span>
          <span className="flow__title">{step.title}</span>
          <span className="flow__note">{step.note}</span>
        </li>
      ))}
    </ol>
  );
}

function ListIcon() {
  return (
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <path d="M3 5h12M3 9h12M3 13h7" />
    </svg>
  );
}

function ShieldIcon() {
  return (
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <path d="M9 2.2l5.4 2.3v4.2c0 3.3-2.2 5.8-5.4 7.1-3.2-1.3-5.4-3.8-5.4-7.1V4.5z" />
      <path d="M6.6 9l1.7 1.7 3.3-3.4" />
    </svg>
  );
}

function TrashIcon() {
  return (
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <path d="M3.4 5.1h11.2M7.3 5.1V3.4h3.4v1.7M4.8 5.1l.7 9.4c.05.7.6 1.3 1.3 1.3h4.4c.7 0 1.25-.6 1.3-1.3l.7-9.4" />
    </svg>
  );
}

function UndoIcon() {
  return (
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <path d="M3.2 9a5.8 5.8 0 1 0 1.8-4.2" />
      <path d="M3 2.6v3.1h3.1" />
    </svg>
  );
}
