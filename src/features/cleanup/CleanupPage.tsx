import { useState } from "react";

import {
  ApplicationsIcon,
  BoxIcon,
  CleanupIcon,
  ClockIcon,
  CodeIcon,
  DocumentIcon,
  DownloadIcon,
  DuplicatesIcon,
  FileIcon,
  LeftBehindIcon,
} from "../../components/icons";
import {
  FindingCard,
  GlassTile,
  Hero,
  PageHeader,
  findingLayout,
} from "../../components/layout";
import { Button, EmptyState, Note, Panel, Safety, Spinner } from "../../components/primitives";
import { formatBytes, formatCount, truncatePath } from "../../lib/format";
import { engineAvailable } from "../../lib/ipc";
import type { CandidateGroup, CandidateKind, CleanupCandidate } from "../../lib/types";

import { RemovalReportNote } from "./Queue";
import type { CleanupController } from "./useCleanup";

/**
 * Cleanup.
 *
 * First a sentence and a card per kind of finding, so the whole picture fits
 * on one screen. Opening a card shows every item in it, each with the reason
 * DiskMate proposed it. Nothing is removed from here: items go to the queue,
 * and the queue is the only thing with a remove button.
 */
export function CleanupPage({
  scanId,
  cleanup,
  onGoToStorage,
}: {
  scanId: number | null;
  cleanup: CleanupController;
  onGoToStorage: () => void;
}) {
  // Null shows the cards; "all" every group; otherwise one group's key.
  const [open, setOpen] = useState<string | null>(null);

  if (!engineAvailable()) {
    return (
      <EmptyState title="The DiskMate engine is not running">
        Start the application with <code className="mono">npm run dev</code>.
      </EmptyState>
    );
  }

  if (scanId === null) {
    return (
      <div className="page">
        <PageHeader title="Cleanup" />
        <Hero
          title="Cleanup works from a scan."
          subtitle="There is nothing to explain until DiskMate has looked at a volume."
          action={
            <Button variant="primary" onClick={onGoToStorage}>
              Go to Storage
            </Button>
          }
        />
      </div>
    );
  }

  const groups = cleanup.groups;
  const shown =
    open === null
      ? []
      : open === "all"
        ? groups
        : groups.filter((group) => groupKey(group) === open);

  if (open !== null) {
    return (
      <div className="page">
        <PageHeader
          title="Cleanup"
          back={{ label: "All findings", onClick: () => setOpen(null) }}
        />
        <div className="stack">
          {cleanup.error && (
            <Note tone="caution" title="That did not work.">
              {cleanup.error}
            </Note>
          )}
          <RemovalReportNote cleanup={cleanup} />
          {shown.map((group) => (
            <GroupPanel key={groupKey(group)} group={group} cleanup={cleanup} />
          ))}
        </div>
      </div>
    );
  }

  const total = groups.reduce((sum, group) => sum + group.bytes, 0);

  return (
    <div className="page">
      <PageHeader
        title="Cleanup"
        back={{ label: "Look again", onClick: () => void cleanup.refresh() }}
      />

      {cleanup.error && (
        <Note tone="caution" title="That did not work.">
          {cleanup.error}
        </Note>
      )}
      <RemovalReportNote cleanup={cleanup} />

      {cleanup.busy && groups.length === 0 ? (
        <div className="page__center">
          <Spinner />
        </div>
      ) : groups.length === 0 ? (
        <Hero
          title="Nothing worth suggesting."
          subtitle="DiskMate found no caches, build output, old downloads or long-untouched files large enough to be worth your attention. That is a good result."
        />
      ) : (
        <>
          <Hero
            title={
              <>
                DiskMate can explain <span className="num">{formatBytes(total)}</span> on your
                Mac.
              </>
            }
            subtitle="Nothing is removed by looking. Open a group to see every item and why it is there."
            action={<Button onClick={() => setOpen("all")}>Review All Findings</Button>}
          />
          <section className="findings" aria-label="Findings">
            {groups.map((group, index) => (
              <FindingCard
                key={groupKey(group)}
                layout={findingLayout(index, groups.length)}
                title={
                  <>
                    <span className="num">{formatBytes(group.bytes)}</span> {phrase(group)}
                  </>
                }
                body={group.summary}
                emblem={
                  <GlassTile
                    size={findingLayout(index, groups.length) === "tall" ? "lg" : "md"}
                  >
                    <KindIcon kind={group.kind} />
                  </GlassTile>
                }
                footnote={
                  <>
                    <Safety safety={group.safety} />
                    <span className="num">{formatCount(group.count)} items</span>
                  </>
                }
                actions={
                  <Button
                    variant={index === 0 ? "primary" : "default"}
                    onClick={() => setOpen(groupKey(group))}
                  >
                    Review
                  </Button>
                }
              />
            ))}
          </section>
        </>
      )}
    </div>
  );
}

function groupKey(group: CandidateGroup): string {
  return `${group.kind}-${group.safety}`;
}

/** How a group reads after its size: "6.1 GB of caches". */
function phrase(group: CandidateGroup): string {
  switch (group.kind) {
    case "trash":
      return "in the Trash";
    case "bigAndUntouched":
      return "in big, untouched files";
    case "duplicateCopy":
      return "in extra copies";
    default:
      return `of ${group.label.charAt(0).toLowerCase()}${group.label.slice(1)}`;
  }
}

function KindIcon({ kind }: { kind: CandidateKind }) {
  const size = 24;
  switch (kind) {
    case "trash":
      return <CleanupIcon size={size} />;
    case "cache":
      return <BoxIcon size={size} />;
    case "developerArtifact":
      return <CodeIcon size={size} />;
    case "oldDownload":
      return <DownloadIcon size={size} />;
    case "bigAndUntouched":
      return <ClockIcon size={size} />;
    case "duplicateCopy":
      return <DuplicatesIcon size={size} />;
    case "appLeftover":
      return <LeftBehindIcon size={size} />;
    case "appBundle":
      return <ApplicationsIcon size={size} />;
    case "log":
      return <DocumentIcon size={size} />;
    default:
      return <FileIcon size={size} />;
  }
}

function GroupPanel({ group, cleanup }: { group: CandidateGroup; cleanup: CleanupController }) {
  const items = cleanup.candidates.filter(
    (candidate) => candidate.kind === group.kind && candidate.safety === group.safety,
  );
  const unstaged = items.filter((item) => !cleanup.stagedIds.has(item.id));
  return (
    <Panel
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
}

function CandidateRow({
  candidate,
  cleanup,
}: {
  candidate: CleanupCandidate;
  cleanup: CleanupController;
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
