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
import { Hero, PageHeader } from "../../components/layout";
import { Button, EmptyState, Note, Panel, Safety, Spinner } from "../../components/primitives";
import { formatBytes, formatCount, truncatePath } from "../../lib/format";
import { engineAvailable } from "../../lib/ipc";
import type { CandidateGroup, CandidateKind, CleanupCandidate } from "../../lib/types";

import { DuplicatesView } from "../duplicates/DuplicatesPage";

import { RemovalReportNote } from "./Queue";
import type { CleanupController } from "./useCleanup";

/**
 * Cleanup.
 *
 * First a sentence and a card per kind of finding, so the whole picture fits
 * on one screen. Opening a card shows every item in it, each with the reason
 * Mr Clean proposed it. Nothing is removed from here: items go to the queue,
 * and the queue is the only thing with a remove button.
 */
export function CleanupPage({
  scanId,
  totalFiles,
  cleanup,
  showDuplicates,
  onShowDuplicates,
  onGoToStorage,
}: {
  scanId: number | null;
  totalFiles: number | null;
  cleanup: CleanupController;
  /** Duplicates lives here: its row opens the full duplicates view. */
  showDuplicates: boolean;
  onShowDuplicates: (show: boolean) => void;
  onGoToStorage: () => void;
}) {
  // Null shows the cards; "all" every group; otherwise one group's key.
  const [open, setOpen] = useState<string | null>(null);

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
        <PageHeader title="Cleanup" />
        <Hero
          title="Cleanup works from a scan."
          subtitle="There is nothing to explain until Mr Clean has looked at a volume."
          action={
            <Button variant="primary" onClick={onGoToStorage}>
              Go to Storage
            </Button>
          }
        />
      </div>
    );
  }

  if (showDuplicates) {
    return (
      <DuplicatesView
        scanId={scanId}
        totalFiles={totalFiles}
        cleanup={cleanup}
        onBack={() => onShowDuplicates(false)}
      />
    );
  }

  const groups = cleanup.groups;
  const hasCopies = groups.some((group) => group.kind === "duplicateCopy");
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
          subtitle="Mr Clean found no caches, build output, old downloads or long-untouched files large enough to be worth your attention. That is a good result."
        />
      ) : (
        <>
          <section className="reclaim-head">
            <div>
              <p className="reclaim-head__kicker">Found in the latest scan</p>
              <p className="reclaim-head__figure num">{formatBytes(total)}</p>
              <p className="reclaim-head__lede">
                is taken up by things Mr Clean can explain. Nothing is removed by looking: open
                a group to see every item and why it is there.
              </p>
            </div>
            <Button onClick={() => setOpen("all")}>Review All</Button>
          </section>
          <section className="finding-list" aria-label="Findings">
            {groups.map((group, index) => (
              <div key={groupKey(group)} className="finding-row">
                <span className="finding-row__tile" aria-hidden="true">
                  <KindIcon kind={group.kind} />
                </span>
                <div className="finding-row__text">
                  <div className="finding-row__name">{group.label}</div>
                  <div className="finding-row__why">{group.summary}</div>
                </div>
                <Safety safety={group.safety} />
                <div className="finding-row__size">
                  <div
                    className={
                      group.kind === "duplicateCopy" ? "tone-copies num" : "tone-reclaim num"
                    }
                  >
                    {formatBytes(group.bytes)}
                  </div>
                  <div className="finding-row__count num">{formatCount(group.count)} items</div>
                </div>
                <Button
                  variant={index === 0 ? "primary" : "default"}
                  aria-label={`Review ${group.label}`}
                  onClick={() =>
                    group.kind === "duplicateCopy"
                      ? onShowDuplicates(true)
                      : setOpen(groupKey(group))
                  }
                >
                  Review
                </Button>
              </div>
            ))}
            {!hasCopies && (
              <div className="finding-row">
                <span className="finding-row__tile" aria-hidden="true">
                  <DuplicatesIcon size={24} />
                </span>
                <div className="finding-row__text">
                  <div className="finding-row__name">Duplicates</div>
                  <div className="finding-row__why">
                    Files stored more than once, compared byte for byte. Not searched yet.
                  </div>
                </div>
                <span />
                <div className="finding-row__size">
                  <div className="num">—</div>
                  <div className="finding-row__count">not compared</div>
                </div>
                <Button onClick={() => onShowDuplicates(true)}>Find</Button>
              </div>
            )}
          </section>
        </>
      )}
    </div>
  );
}

function groupKey(group: CandidateGroup): string {
  return `${group.kind}-${group.safety}`;
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
