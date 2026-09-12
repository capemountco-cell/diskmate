import { useEffect } from "react";

import {
  Badge,
  Button,
  EmptyState,
  Note,
  Panel,
  Safety,
  Segmented,
  Spinner,
} from "../../components/primitives";
import { formatBytes, formatCount, formatRelative, truncatePath } from "../../lib/format";
import { engine, engineAvailable } from "../../lib/ipc";
import type { CleanupCandidate } from "../../lib/types";

import { useCleanup, type CleanupTab } from "./useCleanup";

/**
 * Cleanup.
 *
 * The shape of this screen is the argument: findings on the left, the queue on
 * the right, and the queue is the only thing with a remove button. Nothing is
 * removed by browsing, and the total you approve is the total you were shown.
 */
export function CleanupPage({ scanId }: { scanId: number | null }) {
  const cleanup = useCleanup(scanId);

  useEffect(() => {
    if (cleanup.tab === "applications" && cleanup.applications.length === 0) {
      void cleanup.loadApplications();
    }
    // Loading the application list is a filesystem walk, so it happens when the
    // tab is first opened rather than on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cleanup.tab]);

  if (!engineAvailable()) {
    return (
      <EmptyState title="The DiskMate engine is not running">
        Start the application with <code className="mono">npm run dev</code>.
      </EmptyState>
    );
  }

  if (scanId === null) {
    return (
      <EmptyState title="Scan a volume first">
        Cleanup works from what a scan found, so there is nothing to suggest until one has run.
        Go to Storage and scan a volume.
      </EmptyState>
    );
  }

  return (
    <div className="cleanup-grid">
      <div className="stack">
        {cleanup.error && (
          <Note tone="caution" title="That did not work.">
            {cleanup.error}
          </Note>
        )}

        {cleanup.report && (
          <Note
            tone={cleanup.report.failedCount > 0 ? "caution" : "info"}
            title={`Moved ${formatCount(cleanup.report.removedCount)} items to the Trash, freeing ${formatBytes(cleanup.report.removedBytes)}.`}
          >
            {cleanup.report.refusedCount > 0 && `${cleanup.report.refusedCount} were refused. `}
            {cleanup.report.failedCount > 0 &&
              `${cleanup.report.failedCount} could not be moved. `}
            Everything removed is still in your Trash until you empty it.{" "}
            <button
              type="button"
              className="button button--quiet"
              onClick={cleanup.dismissReport}
            >
              Dismiss
            </button>
          </Note>
        )}

        <div className="row row--between">
          <Segmented<CleanupTab>
            label="Cleanup views"
            value={cleanup.tab}
            onChange={cleanup.setTab}
            options={[
              { value: "suggestions", label: "Suggestions" },
              { value: "duplicates", label: "Duplicates" },
              { value: "applications", label: "Applications" },
            ]}
          />
          <Button onClick={cleanup.refresh} disabled={cleanup.busy}>
            Look again
          </Button>
        </div>

        {cleanup.tab === "suggestions" && <Suggestions cleanup={cleanup} />}
        {cleanup.tab === "duplicates" && <Duplicates cleanup={cleanup} />}
        {cleanup.tab === "applications" && <Applications cleanup={cleanup} />}
      </div>

      <Queue cleanup={cleanup} />
    </div>
  );
}

type Controller = ReturnType<typeof useCleanup>;

function Suggestions({ cleanup }: { cleanup: Controller }) {
  if (cleanup.busy && cleanup.groups.length === 0) return <Spinner />;
  if (cleanup.groups.length === 0) {
    return (
      <Panel>
        <EmptyState title="Nothing worth suggesting">
          DiskMate found no caches, build output, old downloads or long-untouched files large
          enough to be worth your attention. That is a good result.
        </EmptyState>
      </Panel>
    );
  }

  return (
    <div className="stack">
      {cleanup.groups.map((group) => {
        const items = cleanup.candidates.filter(
          (candidate) => candidate.kind === group.kind && candidate.safety === group.safety,
        );
        const unstaged = items.filter((item) => !cleanup.stagedIds.has(item.id));
        return (
          <Panel
            key={`${group.kind}-${group.safety}`}
            title={
              <span className="row">
                {group.label} <Safety safety={group.safety} />
              </span>
            }
            note={group.summary}
            action={
              unstaged.length > 0 && (
                <Button onClick={() => cleanup.stage(unstaged.map((item) => item.id))}>
                  Add {formatCount(unstaged.length)} ·{" "}
                  {formatBytes(unstaged.reduce((sum, item) => sum + item.bytes, 0))}
                </Button>
              )
            }
            flush
          >
            {items.slice(0, 12).map((candidate) => (
              <CandidateRow key={candidate.id} candidate={candidate} cleanup={cleanup} />
            ))}
            {items.length > 12 && (
              <p className="faint" style={{ padding: "var(--space-3) var(--space-4)" }}>
                and {formatCount(items.length - 12)} more, included when you add the group.
              </p>
            )}
          </Panel>
        );
      })}
    </div>
  );
}

function CandidateRow({
  candidate,
  cleanup,
}: {
  candidate: CleanupCandidate;
  cleanup: Controller;
}) {
  const staged = cleanup.stagedIds.has(candidate.id);
  return (
    <div className="candidate">
      <input
        type="checkbox"
        checked={staged}
        aria-label={`Add ${candidate.name} to the queue`}
        onChange={() =>
          staged ? cleanup.unstage([candidate.id]) : cleanup.stage([candidate.id])
        }
      />
      <div style={{ minWidth: 0 }}>
        <div className="table__name-text">{candidate.name}</div>
        <div className="candidate__reason">{candidate.reason}</div>
        {candidate.detail && <div className="candidate__path">{candidate.detail}</div>}
        <div className="candidate__path mono" title={candidate.path}>
          {truncatePath(candidate.path, 78)}
        </div>
      </div>
      <span className="candidate__size">{formatBytes(candidate.bytes)}</span>
      <Safety safety={candidate.safety} />
    </div>
  );
}

function Duplicates({ cleanup }: { cleanup: Controller }) {
  const { duplicateProgress: progress, duplicateStats: stats } = cleanup;

  return (
    <div className="stack">
      <Panel
        title="Duplicates"
        note="Confirmed byte for byte, never by filename."
        action={
          progress ? (
            <Button variant="quiet" onClick={() => void engine.cancelDuplicateSearch()}>
              Cancel
            </Button>
          ) : (
            <Button variant="primary" onClick={cleanup.findDuplicates}>
              {cleanup.duplicates.length > 0 ? "Search again" : "Find duplicates"}
            </Button>
          )
        }
      >
        {progress ? (
          <div className="stack" style={{ gap: "var(--space-2)" }}>
            <div className="scan-bar__track">
              <div className="scan-bar__indicator" />
            </div>
            <div className="scan-bar__stats">
              <span>{progress.stage}</span>
              {progress.total > 0 && (
                <span>
                  {formatCount(progress.done)} of {formatCount(progress.total)}
                </span>
              )}
            </div>
          </div>
        ) : stats ? (
          <div className="stat-row">
            <div>
              <div className="stat__label">Duplicate sets</div>
              <div className="stat__value stat__value--small">{formatCount(stats.groups)}</div>
            </div>
            <div>
              <div className="stat__label">Held twice</div>
              <div className="stat__value stat__value--small">
                {formatBytes(stats.reclaimableBytes)}
              </div>
            </div>
            <div>
              <div className="stat__label">Read to confirm</div>
              <div className="stat__value stat__value--small">
                {formatBytes(stats.bytesRead)}
              </div>
              <div className="stat__hint">
                of {formatCount(stats.sizeCandidates)} files that shared a size
              </div>
            </div>
          </div>
        ) : (
          <p className="muted">
            DiskMate groups files by size first, then reads a sample, and only reads a whole
            file when it still looks identical. Most of your disk is never opened.
          </p>
        )}
      </Panel>

      {cleanup.duplicates.map((group) => (
        <Panel
          key={group.id}
          title={`${formatCount(group.memberCount)} identical copies`}
          note={`${formatBytes(group.logicalSize)} each · ${formatBytes(group.reclaimable)} held more than once`}
          flush
        >
          {group.members.map((member) => (
            <div className="candidate" key={member.entryId}>
              <span style={{ width: 13 }} />
              <div style={{ minWidth: 0 }}>
                <div className="table__name-text">{member.name}</div>
                <div className="candidate__path mono" title={member.path}>
                  {truncatePath(member.path, 78)}
                </div>
              </div>
              <span className="candidate__size faint">{formatRelative(member.modifiedAt)}</span>
              {member.isKeeper ? (
                <Badge tone="accent">Keeping</Badge>
              ) : (
                <Badge>Extra copy</Badge>
              )}
            </div>
          ))}
        </Panel>
      ))}

      {cleanup.duplicates.length > 0 && (
        <Note tone="info" title="The oldest copy is kept.">
          The extra copies appear in Suggestions under Duplicates, where you can add them to the
          queue. The copy being kept is never proposed.
        </Note>
      )}
    </div>
  );
}

function Applications({ cleanup }: { cleanup: Controller }) {
  if (cleanup.loadingApps) return <Spinner />;

  return (
    <div className="stack">
      <Note tone="info" title="Uninstalling means the bundle and everything it scattered.">
        Dragging an app to the Trash leaves its caches, preferences, containers and saved state
        behind. DiskMate finds those by the app's bundle identifier, and adds them to the queue
        for you to review. Applications that came with macOS are not listed.
      </Note>

      <Panel
        title="Installed applications"
        note="Largest first, counting what each one left elsewhere."
        action={<Button onClick={cleanup.loadApplications}>Rescan</Button>}
        flush
      >
        {cleanup.applications.length === 0 ? (
          <EmptyState title="No applications found">
            DiskMate looks in Applications and your home Applications folder.
          </EmptyState>
        ) : (
          cleanup.applications.slice(0, 60).map((app) => (
            <div className="app-row" key={app.id}>
              <div style={{ minWidth: 0 }}>
                <div className="table__name-text">
                  {app.name}
                  {app.version && <span className="faint"> {app.version}</span>}
                </div>
                <div className="app-row__meta">
                  {formatBytes(app.bundleBytes)} app
                  {app.leftoverCount > 0 &&
                    ` · ${formatBytes(app.leftoverBytes)} in ${formatCount(app.leftoverCount)} other places`}
                  {app.bundleId && ` · ${app.bundleId}`}
                </div>
              </div>
              <span className="candidate__size">
                {formatBytes(app.bundleBytes + app.leftoverBytes)}
              </span>
              <Button onClick={() => cleanup.uninstall(app.id)}>Add to queue</Button>
            </div>
          ))
        )}
      </Panel>
    </div>
  );
}

/**
 * The queue.
 *
 * The only place anything is removed, and the reason the rest of the screen can
 * be browsed without consequence.
 */
function Queue({ cleanup }: { cleanup: Controller }) {
  const empty = cleanup.staged.length === 0;

  return (
    <div className="queue">
      <Panel
        title="Queue"
        note={empty ? "Nothing selected yet." : "Review this, then remove it."}
      >
        <div className="queue__total">{formatBytes(cleanup.stagedTotal)}</div>
        <div className="stat__hint">
          {empty
            ? "Add things from the left and they appear here."
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
              <Button
                variant="primary"
                onClick={cleanup.removeApproved}
                disabled={cleanup.busy}
              >
                Move to Trash
              </Button>
            </div>

            <p className="stat__hint" style={{ marginTop: "var(--space-3)" }}>
              Everything goes to the Trash, so you can put it back. DiskMate removes exactly
              what is listed here and nothing else.
            </p>
          </>
        )}
      </Panel>
    </div>
  );
}
