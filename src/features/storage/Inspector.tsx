import { useEffect, useState } from "react";

import { CATEGORY_COLORS } from "../../components/Meter";
import { Button, Safety } from "../../components/primitives";
import {
  formatBytes,
  formatCount,
  formatPercent,
  formatRelative,
  truncatePath,
} from "../../lib/format";
import { describeError, engine } from "../../lib/ipc";
import type { CleanupCandidate, EntryRow } from "../../lib/types";

/** Children fetched for the breakdown. Enough to fill the panel, not the disk. */
const INSIDE_LIMIT = 6;

/**
 * The inspector.
 *
 * Every storage tool has a details panel, and most of them are a table of
 * numbers you could have worked out yourself. The part that earns the space is
 * the last section: what DiskMate is prepared to say about the thing you have
 * selected, and — just as often — that it has nothing to say. A folder with no
 * finding gets told so explicitly, because silence in a cleaner usually means
 * "we haven't looked".
 */
export function Inspector({
  scanId,
  entry,
  scanBytes,
  onOpen,
}: {
  scanId: number;
  entry: EntryRow | null;
  scanBytes: number;
  onOpen: (row: EntryRow) => void;
}) {
  const [inside, setInside] = useState<EntryRow[]>([]);
  const [insideTotal, setInsideTotal] = useState(0);
  const [findings, setFindings] = useState<CleanupCandidate[]>([]);
  const [error, setError] = useState<string | null>(null);

  // The rules are index-only SQL and the write is an upsert, so running them
  // once per scan here is cheap and keeps the inspector from being empty until
  // the user happens to visit Cleanup.
  //
  // Deliberately no "already ran" ref: React invokes this effect twice on
  // mount in development, and a ref set before the request resolves cancels
  // the first call and skips the second, which leaves the panel permanently
  // saying it has nothing to suggest. Running an idempotent upsert twice is
  // the cheaper mistake.
  useEffect(() => {
    let cancelled = false;
    engine
      .proposeCleanup(scanId)
      .then((rows) => !cancelled && setFindings(rows))
      .catch((cause) => !cancelled && setError(describeError(cause)));
    return () => {
      cancelled = true;
    };
  }, [scanId]);

  const entryId = entry?.id ?? null;
  const isFolder = entry?.kind === "directory";
  useEffect(() => {
    if (entryId === null || !isFolder) {
      setInside([]);
      setInsideTotal(0);
      return;
    }
    let cancelled = false;
    engine
      .listChildren({
        scanId,
        parentId: entryId,
        sort: "size",
        direction: "desc",
        limit: INSIDE_LIMIT,
        offset: 0,
        hideHidden: false,
      })
      .then((page) => {
        if (cancelled) return;
        setInside(page.rows);
        setInsideTotal(page.total);
      })
      .catch((cause) => !cancelled && setError(describeError(cause)));
    return () => {
      cancelled = true;
    };
  }, [scanId, entryId, isFolder]);

  if (!entry) {
    return (
      <aside className="inspector">
        <div className="inspector__empty">
          <p className="muted">Select a rectangle or a row.</p>
          <p className="faint">
            Whatever you pick, this panel says what it is, what is inside it, and whether
            DiskMate has anything to suggest about it.
          </p>
        </div>
      </aside>
    );
  }

  const color = CATEGORY_COLORS[entry.category] ?? "var(--cat-unknown)";
  const share = scanBytes > 0 ? entry.size / scanBytes : 0;
  // A candidate counts as "inside" only on a path-segment boundary, so
  // /Users/ada/Documents-old never counts against /Users/ada/Documents.
  const prefix = entry.path.replace(/\/+$/, "");
  const mine = findings.filter(
    (candidate) => candidate.path === prefix || candidate.path.startsWith(`${prefix}/`),
  );
  const reclaimable = mine.reduce((sum, candidate) => sum + candidate.bytes, 0);
  const widest = Math.max(1, ...inside.map((row) => row.size));

  return (
    <aside className="inspector">
      <header className="inspector__head">
        <div className="inspector__title">
          <span className="inspector__swatch" style={{ background: color }} />
          <div style={{ minWidth: 0 }}>
            <div className="inspector__name">{entry.name}</div>
            <div className="inspector__path mono" title={entry.path}>
              {truncatePath(entry.path, 44)}
            </div>
          </div>
        </div>

        <div className="inspector__size">
          <span className="inspector__bytes">{formatBytes(entry.size)}</span>
          <span className="muted">{formatPercent(share)} of the scan</span>
        </div>
        <div className="inspector__share">
          <div style={{ width: `${Math.min(100, share * 100)}%`, background: color }} />
        </div>
      </header>

      <section className="inspector__section inspector__section--sunken">
        <h3 className="inspector__label">What DiskMate can say</h3>
        {mine.length === 0 ? (
          <p className="muted" style={{ fontSize: "var(--text-sm)" }}>
            Nothing to suggest here. DiskMate does not recognise anything inside this as
            regeneratable, duplicated or long abandoned, so it has no opinion about it.
          </p>
        ) : (
          <>
            <div className="inspector__split">
              <div style={{ flex: reclaimable, background: "var(--accent)" }} />
              <div style={{ flex: Math.max(0, entry.size - reclaimable) }} />
            </div>
            <div className="inspector__splitkey">
              <span>
                <i style={{ background: "var(--accent)" }} />
                Can explain {formatBytes(reclaimable)}
              </span>
              {entry.size > reclaimable && (
                <span>
                  <i style={{ background: "var(--grey-4)" }} />
                  Yours {formatBytes(entry.size - reclaimable)}
                </span>
              )}
            </div>
            <ul className="inspector__findings">
              {mine.slice(0, 3).map((candidate) => (
                <li key={candidate.id}>
                  <div className="row" style={{ gap: "var(--space-2)" }}>
                    <span className="inspector__finding-name">{candidate.name}</span>
                    <Safety safety={candidate.safety} />
                    <span className="faint num" style={{ marginLeft: "auto" }}>
                      {formatBytes(candidate.bytes)}
                    </span>
                  </div>
                  <p className="inspector__reason">{candidate.reason}</p>
                </li>
              ))}
            </ul>
            {mine.length > 3 && (
              <p className="faint" style={{ fontSize: "var(--text-xs)" }}>
                and {formatCount(mine.length - 3)} more, on the Cleanup page.
              </p>
            )}
          </>
        )}
      </section>

      <section className="inspector__section">
        <h3 className="inspector__label">Details</h3>
        <dl className="inspector__details">
          <Detail term="On disk" value={formatBytes(entry.allocatedSize)} />
          <Detail term="Logical size" value={formatBytes(entry.size)} />
          {isFolder && <Detail term="Files" value={formatCount(entry.fileCount)} />}
          {isFolder && <Detail term="Folders" value={formatCount(entry.dirCount)} />}
          <Detail
            term="Last changed"
            value={
              entry.modifiedAt === null ? "Not recorded" : formatRelative(entry.modifiedAt)
            }
          />
          {entry.sharesStorage && (
            <Detail term="Shared storage" value="Another name points at these same bytes" />
          )}
          {!entry.accessible && (
            <Detail term="Not readable" value="Contents are missing from the index" caution />
          )}
        </dl>
      </section>

      {isFolder && (
        <section className="inspector__section inspector__section--grow">
          <div className="row row--between">
            <h3 className="inspector__label">Largest inside</h3>
            <span className="faint num" style={{ fontSize: "var(--text-xs)" }}>
              of {formatCount(insideTotal)}
            </span>
          </div>
          {inside.length === 0 ? (
            <p className="muted" style={{ fontSize: "var(--text-sm)" }}>
              {entry.accessible
                ? "This folder is empty."
                : "DiskMate could not read inside it."}
            </p>
          ) : (
            <ul className="inspector__inside">
              {inside.map((row) => (
                <li key={row.id}>
                  <button
                    type="button"
                    className="inspector__inside-row"
                    onDoubleClick={row.kind === "directory" ? () => onOpen(row) : undefined}
                  >
                    <span className="row" style={{ gap: 7, minWidth: 0 }}>
                      <i
                        className="inspector__dot"
                        style={{
                          background: CATEGORY_COLORS[row.category] ?? "var(--cat-unknown)",
                        }}
                      />
                      <span className="inspector__inside-name">{row.name}</span>
                    </span>
                    <span className="faint num">{formatBytes(row.size)}</span>
                  </button>
                  <div className="inspector__inside-bar">
                    <div
                      style={{
                        width: `${(row.size / widest) * 100}%`,
                        background: CATEGORY_COLORS[row.category] ?? "var(--cat-unknown)",
                      }}
                    />
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      <footer className="inspector__foot">
        {error && <p className="faint">{error}</p>}
        <div className="row" style={{ gap: "var(--space-2)" }}>
          <Button
            onClick={() => void reveal(entry.path)}
            style={{ flex: 1, justifyContent: "center" }}
          >
            Reveal in Finder
          </Button>
          <Button
            onClick={() => void copy(entry.path)}
            style={{ flex: 1, justifyContent: "center" }}
          >
            Copy path
          </Button>
        </div>
      </footer>
    </aside>
  );
}

function Detail({ term, value, caution }: { term: string; value: string; caution?: boolean }) {
  return (
    <>
      <dt>{term}</dt>
      <dd className={caution ? "num inspector__caution" : "num"}>{value}</dd>
    </>
  );
}

/** Opens Finder with the item selected. The window is granted exactly this. */
async function reveal(path: string): Promise<void> {
  try {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path);
  } catch {
    // Refusing to reveal is not worth an error banner; the path is on screen
    // and the Copy button is next to it.
  }
}

async function copy(path: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(path);
  } catch {
    // Same: the path is already legible in the panel.
  }
}
