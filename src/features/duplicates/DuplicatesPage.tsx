import { DuplicatesIcon } from "../../components/icons";
import { FindingCard, GlassTile, Hero, PageHeader } from "../../components/layout";
import { Badge, Button, EmptyState, Note, Panel } from "../../components/primitives";
import { formatBytes, formatCount, formatRelative, truncatePath } from "../../lib/format";
import { engine, engineAvailable } from "../../lib/ipc";
import type { DuplicateStats } from "../../lib/types";
import { RemovalReportNote } from "../cleanup/Queue";
import type { CleanupController } from "../cleanup/useCleanup";

/**
 * Duplicates.
 *
 * Its own section because finding them is its own piece of work: Mr Clean
 * reads file contents here, which no other screen does, and the cost of that
 * is shown alongside the answer.
 */
export function DuplicatesPage({
  scanId,
  totalFiles,
  cleanup,
  onGoToStorage,
}: {
  scanId: number | null;
  totalFiles: number | null;
  cleanup: CleanupController;
  onGoToStorage: () => void;
}) {
  const { duplicateProgress: progress, duplicateStats: stats, duplicates } = cleanup;

  if (!engineAvailable()) {
    return (
      <EmptyState title="The Mr Clean engine is not running">
        Start the application with <code className="mono">npm run dev</code>.
      </EmptyState>
    );
  }

  if (scanId === null) {
    return (
      <div className="page">
        <PageHeader title="Duplicates" />
        <Hero
          title="Duplicates are found within a scan."
          subtitle="Scan a volume first, then Mr Clean can compare what it found."
          action={
            <Button variant="primary" onClick={onGoToStorage}>
              Go to Storage
            </Button>
          }
        />
      </div>
    );
  }

  return (
    <div className="page">
      <PageHeader
        title="Duplicates"
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
            <div className="scan-bar__track">
              <div className="scan-bar__indicator" />
            </div>
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
          <Hero
            title={
              stats.groups === 0 ? (
                "No identical files found."
              ) : (
                <>
                  You have <span className="num">{formatCount(stats.groups)}</span> sets of
                  identical files.
                </>
              )
            }
            subtitle={
              stats.groups === 0
                ? "Every file Mr Clean compared turned out to be different."
                : `${formatBytes(stats.reclaimableBytes)} is held more than once. Each set was compared byte for byte, never by name.`
            }
          />

          <section className="findings">
            <FindingCard
              layout="wide"
              title="What Mr Clean read to prove it"
              emblem={
                <GlassTile>
                  <DuplicatesIcon size={24} />
                </GlassTile>
              }
            >
              <Funnel stats={stats} totalFiles={totalFiles} />
            </FindingCard>
          </section>

          {duplicates.map((group) => (
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
                  <span className="candidate__size faint">
                    {formatRelative(member.modifiedAt)}
                  </span>
                  {member.isKeeper ? (
                    <Badge tone="accent">Keeping</Badge>
                  ) : (
                    <Badge>Extra copy</Badge>
                  )}
                </div>
              ))}
            </Panel>
          ))}

          {duplicates.length > 0 && (
            <Note tone="info" title="The oldest copy is kept.">
              The extra copies appear in Cleanup under Duplicates, where you can add them to the
              queue. The copy being kept is never proposed.
            </Note>
          )}
        </>
      )}
    </div>
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
