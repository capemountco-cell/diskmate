import { useEffect, useMemo, useState } from "react";

import { Hero, PageHeader } from "../../components/layout";
import { Button, EmptyState, Note } from "../../components/primitives";
import { formatBytes, formatCount, formatRelative, truncatePath } from "../../lib/format";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import { reveal } from "../../lib/reveal";
import type { CleanupCandidate, DuplicateGroup, DuplicateStats } from "../../lib/types";
import { RemovalReportNote } from "../cleanup/Queue";
import type { CleanupController } from "../cleanup/useCleanup";

/**
 * Duplicates, opened from its row in Cleanup.
 *
 * A view of its own because finding them is its own piece of work: Mr Clean
 * reads file contents here, which nothing else does, and what that cost is
 * shown alongside the answer. Then every set, with the copy being kept marked
 * and a box on every extra copy.
 */
export function DuplicatesView({
  scanId,
  totalFiles,
  cleanup,
  onBack,
}: {
  scanId: number;
  totalFiles: number | null;
  cleanup: CleanupController;
  onBack: () => void;
}) {
  const { duplicateProgress: progress, duplicateStats: stats } = cleanup;

  if (!engineAvailable()) {
    return (
      <EmptyState title="The Mr Clean engine is not running">
        Start the application with <code className="mono">npm run dev</code>.
      </EmptyState>
    );
  }

  return (
    <div className="page">
      <PageHeader
        title="Cleanup"
        back={{ label: "All findings", onClick: onBack }}
        aside={
          progress ? (
            <Button variant="quiet" onClick={() => void engine.cancelDuplicateSearch()}>
              Cancel
            </Button>
          ) : stats ? (
            <Button variant="quiet" onClick={cleanup.findDuplicates}>
              Search again
            </Button>
          ) : null
        }
      />

      {cleanup.error && (
        <Note tone="caution" title="That did not work.">
          {cleanup.error}
        </Note>
      )}
      <RemovalReportNote cleanup={cleanup} />

      {progress ? (
        <>
          <Hero title="Comparing files…" subtitle={progress.stage} />
          <div className="scan-bar">
            {progress.total > 0 && (
              // The comparison does know its total: the files that shared a
              // size. So this one is a true fraction of the work.
              <div
                className="scan-bar__track"
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={progress.total}
                aria-valuenow={progress.done}
                aria-label={`${progress.done} of ${progress.total} compared`}
              >
                <div
                  className="scan-bar__fill"
                  style={{ width: `${Math.min(progress.done / progress.total, 1) * 100}%` }}
                />
              </div>
            )}
            {progress.total > 0 && (
              <div className="scan-bar__stats">
                <span>
                  {formatCount(progress.done)} of {formatCount(progress.total)}
                </span>
              </div>
            )}
          </div>
        </>
      ) : !stats ? (
        <Hero
          title="Find files that are stored more than once."
          subtitle="Mr Clean groups files by size first, then reads a sample, and only reads a whole file when it still looks identical. Most of your disk is never opened."
          action={
            <Button variant="primary" onClick={cleanup.findDuplicates}>
              Find Duplicates
            </Button>
          }
        />
      ) : (
        <>
          <DuplicateResults
            scanId={scanId}
            stats={stats}
            totalFiles={totalFiles}
            cleanup={cleanup}
          />
        </>
      )}
    </div>
  );
}

/**
 * The answer, and what you can do about it.
 *
 * One figure (the space held more than once), one button that queues every
 * extra copy, then each set with the copy being kept marked and a box on every
 * extra copy. The oldest copy is kept and is never offered. Ticking goes to
 * the same queue as everything else, and nothing moves until the queue is
 * reviewed, and then only to the Trash.
 */
function DuplicateResults({
  scanId,
  stats,
  totalFiles,
  cleanup,
}: {
  scanId: number;
  stats: DuplicateStats;
  totalFiles: number | null;
  cleanup: CleanupController;
}) {
  const [copies, setCopies] = useState<CleanupCandidate[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { duplicates, stagedIds } = cleanup;

  // The extra copies become cleanup candidates once proposed; read them so
  // each one on screen can be ticked.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await engine.proposeCleanup(scanId);
        const rows = await engine.cleanupCandidates(scanId, null, 20_000);
        if (!cancelled) setCopies(rows.filter((row) => row.kind === "duplicateCopy"));
      } catch (cause) {
        if (!cancelled) setError(describeError(cause));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [scanId, stats]);

  const byPath = useMemo(
    () => new Map((copies ?? []).map((candidate) => [candidate.path, candidate])),
    [copies],
  );
  const allIds = (copies ?? []).map((candidate) => candidate.id);
  const queued = allIds.filter((id) => stagedIds.has(id));
  const allQueued = allIds.length > 0 && queued.length === allIds.length;
  const queuedBytes = (copies ?? [])
    .filter((candidate) => stagedIds.has(candidate.id))
    .reduce((sum, candidate) => sum + candidate.bytes, 0);

  const idsFor = (group: DuplicateGroup) =>
    group.members
      .filter((member) => !member.isKeeper)
      .map((member) => byPath.get(member.path)?.id)
      .filter((id): id is number => id !== undefined);

  if (stats.groups === 0) {
    return (
      <Hero
        title="No identical files found."
        subtitle="Every file Mr Clean compared turned out to be different."
      />
    );
  }

  return (
    <>
      <header className="page-head">
        <p className="kicker">Duplicates</p>
        <h1 className="headline">
          You have <span className="num">{formatCount(stats.groups)}</span> sets of identical
          files.
        </h1>
        <p className="lede">
          <strong className="num">{formatBytes(stats.reclaimableBytes)}</strong> is held more
          than once. Each set was compared byte for byte, never by name.
        </p>
        <div className="ov__lede-row">
          <p className="lede">
            {queued.length > 0 ? (
              <>
                <strong className="num">{formatCount(queued.length)}</strong> extra copies (
                {formatBytes(queuedBytes)}) are in your queue. The oldest copy of each file is
                always kept.
              </>
            ) : (
              "Tick the extra copies you want gone, or queue them all. The oldest copy of each file is always kept."
            )}
          </p>
          {copies === null ? (
            <Button disabled>Reading copies…</Button>
          ) : allQueued ? (
            <Button onClick={() => void cleanup.unstage(allIds)}>
              Take all out of the queue
            </Button>
          ) : (
            <Button
              variant="primary"
              disabled={allIds.length === 0}
              onClick={() => void cleanup.stage(allIds.filter((id) => !stagedIds.has(id)))}
            >
              Queue every extra copy
            </Button>
          )}
        </div>
      </header>

      {error && <Note tone="caution">{error}</Note>}

      <section className="card" aria-label="What Mr Clean read to prove it">
        <div className="card__head">
          <p className="card__title">What Mr Clean read to prove it</p>
          <p className="card__note">Most files are never opened</p>
        </div>
        <div className="card__chart">
          <Funnel stats={stats} totalFiles={totalFiles} />
        </div>
      </section>

      {duplicates.map((group) => {
        const ids = idsFor(group);
        const groupQueued = ids.length > 0 && ids.every((id) => stagedIds.has(id));
        return (
          <section
            key={group.id}
            className="rows dup-set"
            aria-label={`${group.memberCount} identical copies`}
          >
            <div className="dup-set__head">
              <div>
                <p className="row-item__title">
                  {formatCount(group.memberCount)} identical copies
                </p>
                <p className="row-item__detail">
                  {formatBytes(group.logicalSize)} each · {formatBytes(group.reclaimable)} held
                  more than once
                </p>
              </div>
              {ids.length > 0 &&
                (groupQueued ? (
                  <Button onClick={() => void cleanup.unstage(ids)}>Take out of queue</Button>
                ) : (
                  <Button
                    onClick={() => void cleanup.stage(ids.filter((id) => !stagedIds.has(id)))}
                  >
                    Queue extra copies
                  </Button>
                ))}
            </div>
            {group.members.map((member) => {
              const candidate = member.isKeeper ? undefined : byPath.get(member.path);
              return (
                <div
                  className={
                    candidate && stagedIds.has(candidate.id)
                      ? "dup-row dup-row--picked"
                      : "dup-row"
                  }
                  key={member.entryId}
                >
                  {member.isKeeper ? (
                    <span className="status-chip status-chip--quiet dup-row__keep">
                      Keeping
                    </span>
                  ) : candidate ? (
                    <input
                      type="checkbox"
                      className="box"
                      checked={stagedIds.has(candidate.id)}
                      onChange={() =>
                        void (stagedIds.has(candidate.id)
                          ? cleanup.unstage([candidate.id])
                          : cleanup.stage([candidate.id]))
                      }
                      aria-label={`Add the copy at ${member.path} to the queue`}
                    />
                  ) : (
                    <span className="dup-row__keep faint">Not offered</span>
                  )}
                  <div className="dup-row__text">
                    <p className="dup-row__name">{member.name}</p>
                    <p className="dup-row__path mono" title={member.path}>
                      {truncatePath(member.path, 90)}
                    </p>
                  </div>
                  <span className="faint dup-row__when">
                    {formatRelative(member.modifiedAt)}
                  </span>
                  <button
                    type="button"
                    className="link-button"
                    onClick={() => void reveal(member.path)}
                  >
                    Show in Finder
                  </button>
                </div>
              );
            })}
          </section>
        );
      })}
    </>
  );
}

/**
 * What the duplicate search actually opened.
 *
 * Four disconnected numbers do not say the interesting thing, which is the
 * narrowing: nearly every file leaves at the first stage without being read at
 * all. Showing the stages in order, with the count that survives each one,
 * makes the cost of the answer visible alongside the answer.
 */
function Funnel({ stats, totalFiles }: { stats: DuplicateStats; totalFiles: number | null }) {
  const stages = [
    totalFiles === null
      ? null
      : {
          key: "all",
          count: formatCount(totalFiles),
          label: "files on disk",
          note: "Grouped by size, from the index. Nothing read.",
        },
    {
      key: "size",
      count: formatCount(stats.sizeCandidates),
      label: "shared a size",
      note: "A sample read from each end of the file.",
    },
    {
      key: "full",
      count: formatCount(stats.fullyHashed),
      label: "matched that sample",
      note: "Read in full, start to finish.",
    },
  ].filter((stage) => stage !== null);

  return (
    <div className="funnel">
      <ol className="funnel__stages">
        {stages.map((stage) => (
          <li key={stage.key} className="funnel__stage">
            <span className="funnel__count num">{stage.count}</span>
            <span className="funnel__label">{stage.label}</span>
            <span className="funnel__note">{stage.note}</span>
          </li>
        ))}
        <li className="funnel__stage funnel__stage--result">
          <span className="funnel__count num">{formatCount(stats.groups)}</span>
          <span className="funnel__label">identical sets</span>
          <span className="funnel__note">
            {formatBytes(stats.reclaimableBytes)} held more than once.
          </span>
        </li>
      </ol>

      <p className="funnel__read">
        <b>{formatBytes(stats.bytesRead)} was opened</b> to prove it. A tool that hashed
        everything would read the whole disk to discover that almost none of it matches.
      </p>
    </div>
  );
}
