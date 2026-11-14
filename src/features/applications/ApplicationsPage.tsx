import { useEffect, useMemo, useRef, useState } from "react";

import { Button, EmptyState, Note, Spinner } from "../../components/primitives";
import { formatBytes, formatCount, truncatePath } from "../../lib/format";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import { reveal } from "../../lib/reveal";
import type { Application, CleanupCandidate, StartupReport } from "../../lib/types";
import { RemovalReportNote } from "../cleanup/Queue";
import type { CleanupController } from "../cleanup/useCleanup";
import { StartupTab } from "../startup/StartupPage";

export type AppsTab = "uninstall" | "startup";
type Queued = "all" | "some" | "none";
type Sort = "opened" | "size" | "name";

const DAY = 24 * 60 * 60;

/**
 * Apps.
 *
 * One list. Opening an app shows, in place, every file it would take with it,
 * in three groups: what goes with the app, what to look at first (things you
 * made or saved with it, left unticked), and what Mr Clean will not touch.
 * Ticking an app puts it and its own files in the queue; the bar at the bottom
 * says what the queue holds, and the queue is still the only place anything is
 * removed, and then only to the Trash.
 *
 * Startup sits here as a tab, because what starts with your Mac is put there
 * by apps.
 */
export function ApplicationsPage({
  cleanup,
  tab,
  onTab,
  startup,
  onStartup,
}: {
  cleanup: CleanupController;
  tab: AppsTab;
  onTab: (tab: AppsTab) => void;
  startup: StartupReport | null;
  onStartup: (report: StartupReport) => void;
}) {
  const [sort, setSort] = useState<Sort>("opened");
  const [query, setQuery] = useState("");
  const [openId, setOpenId] = useState<number | null>(null);
  /** What each app would take with it, read when it is first opened or ticked. */
  const [proposals, setProposals] = useState<Record<number, CleanupCandidate[]>>({});
  const [proposalError, setProposalError] = useState<string | null>(null);

  const { applications, loadApplications, loadingApps } = cleanup;

  useEffect(() => {
    if (engineAvailable() && applications.length === 0) void loadApplications();
    // The application list is a filesystem walk, so it runs when the section is
    // first opened rather than when the app starts.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const now = Math.floor(Date.now() / 1000);
  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const shown = needle
      ? applications.filter((app) => app.name.toLowerCase().includes(needle))
      : [...applications];
    if (sort === "size") {
      shown.sort((a, b) => b.bundleBytes + b.leftoverBytes - (a.bundleBytes + a.leftoverBytes));
    } else if (sort === "name") {
      shown.sort((a, b) => a.name.localeCompare(b.name));
    } else {
      // Least recently opened first. Never recorded sorts last: it is
      // unknown, not old.
      shown.sort((a, b) => (a.lastUsedAt ?? Infinity) - (b.lastUsedAt ?? Infinity));
    }
    return shown;
  }, [applications, query, sort]);

  const propose = async (app: Application): Promise<CleanupCandidate[] | null> => {
    const known = proposals[app.id];
    if (known) return known;
    try {
      const rows = await engine.proposeUninstall(app.id);
      setProposals((current) => ({ ...current, [app.id]: rows }));
      setProposalError(null);
      return rows;
    } catch (cause) {
      setProposalError(describeError(cause));
      return null;
    }
  };

  const toggleOpen = (app: Application) => {
    if (openId === app.id) return setOpenId(null);
    setOpenId(app.id);
    void propose(app);
  };

  /**
   * How much of an app is in the queue: all (the app itself), some (only
   * files it left elsewhere), or none.
   */
  const queuedState = (app: Application): Queued => {
    const mine = cleanup.staged.filter(
      (c) => c.path === app.path || c.groupKey === `app:${app.id}`,
    );
    if (mine.some((c) => c.path === app.path)) return "all";
    return mine.length > 0 ? "some" : "none";
  };

  /** Ticking an app queues it and the files only it uses; never "look at first". */
  const togglePicked = async (app: Application) => {
    const rows = await propose(app);
    if (!rows) return;
    if (queuedState(app) !== "none") {
      const ids = rows.filter((c) => cleanup.stagedIds.has(c.id)).map((c) => c.id);
      if (ids.length > 0) await cleanup.unstage(ids);
    } else {
      const ids = rows
        .filter((c) => c.safety !== "protected" && (c.safety === "safe" || c.path === app.path))
        .map((c) => c.id);
      if (ids.length > 0) await cleanup.stage(ids);
    }
  };

  const toggleFile = (candidate: CleanupCandidate) =>
    cleanup.stagedIds.has(candidate.id)
      ? cleanup.unstage([candidate.id])
      : cleanup.stage([candidate.id]);

  if (!engineAvailable()) {
    return (
      <EmptyState title="The Mr Clean engine is not running">
        Start the application with <code className="mono">npm run dev</code>.
      </EmptyState>
    );
  }

  const totalBytes = applications.reduce((sum, app) => sum + app.bundleBytes, 0);
  const userStartup = startup
    ? startup.items.filter((item) => !item.isOsProvided).length
    : null;

  return (
    <div className="page">
      <header className="page-head">
        <p className="kicker">In Applications, with what each app keeps elsewhere</p>
        <p className="bigline">
          {applications.length > 0 ? (
            <>
              <b className="num">{formatCount(applications.length)} apps</b>
              <span>using {formatBytes(totalBytes)}</span>
            </>
          ) : (
            <span>{loadingApps ? "Reading Applications…" : "No apps read yet"}</span>
          )}
        </p>
      </header>

      <div className="toolbar">
        <div className="seg" role="group" aria-label="Apps views">
          <button
            type="button"
            aria-pressed={tab === "uninstall"}
            onClick={() => onTab("uninstall")}
          >
            Uninstall
          </button>
          <button
            type="button"
            aria-pressed={tab === "startup"}
            onClick={() => onTab("startup")}
          >
            Startup
            {userStartup !== null && <small className="num">{userStartup}</small>}
          </button>
        </div>
        {tab === "uninstall" && (
          <div className="toolbar__tools">
            <div className="sort" role="group" aria-label="Sort apps">
              <span>Sort</span>
              <button
                type="button"
                aria-pressed={sort === "opened"}
                onClick={() => setSort("opened")}
              >
                Last opened
              </button>
              <button
                type="button"
                aria-pressed={sort === "size"}
                onClick={() => setSort("size")}
              >
                Size
              </button>
              <button
                type="button"
                aria-pressed={sort === "name"}
                onClick={() => setSort("name")}
              >
                Name
              </button>
            </div>
            <label className="search">
              <svg
                width="14"
                height="14"
                viewBox="0 0 14 14"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.6"
                aria-hidden="true"
              >
                <circle cx="6" cy="6" r="4.2" />
                <path d="m9.2 9.2 3 3" />
              </svg>
              <span className="visually-hidden">Search apps</span>
              <input
                type="search"
                placeholder="Search apps"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>
            <Button variant="quiet" onClick={loadApplications} disabled={loadingApps}>
              Read again
            </Button>
          </div>
        )}
      </div>

      {tab === "startup" ? (
        <StartupTab report={startup} onReport={onStartup} />
      ) : (
        <>
          {cleanup.error && <Note tone="caution">{cleanup.error}</Note>}
          {proposalError && <Note tone="caution">{proposalError}</Note>}
          <RemovalReportNote cleanup={cleanup} />

          {loadingApps && applications.length === 0 ? (
            <Spinner />
          ) : visible.length === 0 ? (
            <div className="rows">
              <p className="rows__empty">
                {applications.length === 0
                  ? "Mr Clean found no apps in Applications or your home Applications folder."
                  : "No app matches that search."}
              </p>
            </div>
          ) : (
            <section className="rows" aria-label="Installed apps">
              {visible.slice(0, 200).map((app) => (
                <AppRow
                  key={app.id}
                  app={app}
                  now={now}
                  open={openId === app.id}
                  queued={queuedState(app)}
                  proposal={proposals[app.id] ?? null}
                  stagedIds={cleanup.stagedIds}
                  onToggleOpen={() => toggleOpen(app)}
                  onTogglePicked={() => void togglePicked(app)}
                  onToggleFile={(candidate) => void toggleFile(candidate)}
                />
              ))}
            </section>
          )}
          <p className="faint">
            Apps that come with macOS are not listed. Ticking an app adds it to the queue;
            nothing moves until you review the queue, and then only to the Trash.
          </p>
        </>
      )}
    </div>
  );
}

function AppRow({
  app,
  now,
  open,
  queued,
  proposal,
  stagedIds,
  onToggleOpen,
  onTogglePicked,
  onToggleFile,
}: {
  app: Application;
  now: number;
  open: boolean;
  queued: Queued;
  proposal: CleanupCandidate[] | null;
  stagedIds: Set<number>;
  onToggleOpen: () => void;
  onTogglePicked: () => void;
  onToggleFile: (candidate: CleanupCandidate) => void;
}) {
  const goes =
    proposal?.filter(
      (c) => c.safety === "safe" || (c.path === app.path && c.safety !== "protected"),
    ) ?? [];
  const look = proposal?.filter((c) => c.safety === "review" && c.path !== app.path) ?? [];
  const kept = proposal?.filter((c) => c.safety === "protected") ?? [];
  const sum = (rows: CleanupCandidate[]) => rows.reduce((n, c) => n + c.bytes, 0);

  const box = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (box.current) box.current.indeterminate = queued === "some";
  }, [queued]);

  const sub = [app.version, formatBytes(app.bundleBytes), describeOpened(app.lastUsedAt, now)]
    .filter(Boolean)
    .join(" · ");

  return (
    <div
      className={[
        "app-row",
        open ? "app-row--open" : "",
        queued !== "none" ? "app-row--picked" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div className="app-row__head">
        <button
          type="button"
          className="app-row__opener"
          aria-expanded={open}
          onClick={onToggleOpen}
        >
          <span className="app-tile" aria-hidden="true">
            {app.name.trim().charAt(0).toUpperCase() || "?"}
          </span>
          <span className="app-row__text">
            <span className="app-row__name">{app.name}</span>
            <span className="app-row__sub">{sub}</span>
          </span>
          <svg
            className="app-row__chev"
            width="12"
            height="12"
            viewBox="0 0 12 12"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            aria-hidden="true"
          >
            <path d="m3 4.5 3 3 3-3" />
          </svg>
        </button>
        <span className="app-row__summary">
          {queued !== "none" && (
            <span className="in-queue">
              {queued === "all" ? "In queue" : "Some files in queue"}
            </span>
          )}
          {proposal ? (
            <>
              Removes <b className="num">{formatBytes(sum(goes))}</b>
              {look.length > 0 && <> · {formatBytes(sum(look))} more to look at</>}
            </>
          ) : app.leftoverBytes > 0 ? (
            <>
              <b className="num">{formatBytes(app.bundleBytes)}</b> app ·{" "}
              {formatBytes(app.leftoverBytes)} elsewhere
            </>
          ) : (
            <>
              <b className="num">{formatBytes(app.bundleBytes)}</b> app · nothing elsewhere
            </>
          )}
        </span>
        <input
          type="checkbox"
          ref={box}
          className="box"
          checked={queued === "all"}
          onChange={onTogglePicked}
          aria-label={`Add ${app.name} and the files only it uses to the queue`}
        />
      </div>

      {open && (
        <div className="app-row__files">
          {proposal === null ? (
            <Spinner />
          ) : (
            <>
              <FileGroup
                title="Goes with the app"
                note="files only it uses"
                rows={goes}
                stagedIds={stagedIds}
                onToggle={onToggleFile}
              />
              {look.length > 0 && (
                <FileGroup
                  title="Look at first"
                  note="things you made or saved with it, kept unless you tick them"
                  rows={look}
                  stagedIds={stagedIds}
                  onToggle={onToggleFile}
                  showWhy
                />
              )}
              {kept.length > 0 && (
                <FileGroup
                  title="Mr Clean will not touch"
                  note="protected"
                  rows={kept}
                  showWhy
                />
              )}
              <div className="row">
                <Button variant="quiet" onClick={() => void reveal(app.path)}>
                  Show in Finder
                </Button>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function FileGroup({
  title,
  note,
  rows,
  stagedIds,
  onToggle,
  showWhy,
}: {
  title: string;
  note: string;
  rows: CleanupCandidate[];
  stagedIds?: Set<number>;
  onToggle?: (candidate: CleanupCandidate) => void;
  showWhy?: boolean;
}) {
  return (
    <div className="file-group">
      <p className="file-group__title">
        {title} <span>· {note}</span>
      </p>
      {rows.length === 0 ? (
        <p className="file-group__empty">Nothing.</p>
      ) : (
        rows.map((candidate) => (
          <div key={candidate.id} className="file-row">
            {onToggle && stagedIds ? (
              <input
                type="checkbox"
                className="box"
                checked={stagedIds.has(candidate.id)}
                onChange={() => onToggle(candidate)}
                aria-label={`Add ${candidate.name} to the queue`}
              />
            ) : (
              <span aria-hidden="true" />
            )}
            <span className="file-row__name">{candidate.name}</span>
            <span className="file-row__path mono" title={candidate.path}>
              {truncatePath(candidate.path, 70)}
            </span>
            <span className="file-row__size num">{formatBytes(candidate.bytes)}</span>
            {showWhy && <p className="file-row__why">{candidate.reason}</p>}
          </div>
        ))
      )}
    </div>
  );
}

function describeOpened(lastUsedAt: number | null, now: number): string {
  if (lastUsedAt === null) return "last opened not recorded";
  const days = Math.max(0, Math.floor((now - lastUsedAt) / DAY));
  if (days < 1) return "opened today";
  if (days < 2) return "opened yesterday";
  if (days < 14) return `opened ${days} days ago`;
  if (days < 60) return `opened ${Math.round(days / 7)} weeks ago`;
  if (days < 365) return `not opened in ${Math.round(days / 30)} months`;
  return "not opened in over a year";
}
