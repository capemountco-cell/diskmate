import { useEffect, useMemo, useState } from "react";

import { ChevronLeftIcon } from "../../components/icons";
import { Button, EmptyState, Note, Safety, Spinner } from "../../components/primitives";
import { formatBytes, formatCount, formatRelative, truncatePath } from "../../lib/format";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import { reveal } from "../../lib/reveal";
import type { Application, Leftover } from "../../lib/types";
import { RemovalReportNote } from "../cleanup/Queue";
import type { CleanupController } from "../cleanup/useCleanup";

type Filter = "all" | "unused" | "leftovers";
type Sort = "size" | "name" | "opened";

/** Six months, the line after which an app counts as not opened lately. */
const UNUSED_AFTER_SECONDS = 183 * 24 * 60 * 60;

/**
 * Applications.
 *
 * A working list rather than a set of cards, so it sits on a plain light panel
 * inside the section's colour: long lists of names and sizes are easier to read
 * dark-on-light. The detail pane shows what an app keeps outside its own
 * folder, read from disk when the app is selected.
 */
export function ApplicationsPage({
  cleanup,
  onBack,
}: {
  cleanup: CleanupController;
  onBack: () => void;
}) {
  const [filter, setFilter] = useState<Filter>("all");
  const [sort, setSort] = useState<Sort>("size");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [leftovers, setLeftovers] = useState<Leftover[] | null>(null);
  const [leftoverError, setLeftoverError] = useState<string | null>(null);

  const { applications, loadApplications, loadingApps } = cleanup;

  useEffect(() => {
    if (engineAvailable() && applications.length === 0) void loadApplications();
    // The application list is a filesystem walk, so it runs when the section is
    // first opened rather than when the app starts.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const now = Math.floor(Date.now() / 1000);
  const visible = useMemo(() => {
    const filtered = applications.filter((app) => {
      if (filter === "unused") {
        return app.lastUsedAt !== null && now - app.lastUsedAt > UNUSED_AFTER_SECONDS;
      }
      if (filter === "leftovers") return app.leftoverBytes > 0;
      return true;
    });
    const sorted = [...filtered];
    if (sort === "size") {
      sorted.sort(
        (a, b) => b.bundleBytes + b.leftoverBytes - (a.bundleBytes + a.leftoverBytes),
      );
    } else if (sort === "name") {
      sorted.sort((a, b) => a.name.localeCompare(b.name));
    } else {
      // Never-recorded last use sorts last: it is unknown, not old.
      sorted.sort((a, b) => (a.lastUsedAt ?? Infinity) - (b.lastUsedAt ?? Infinity));
    }
    return sorted;
  }, [applications, filter, sort, now]);

  const selected: Application | null =
    visible.find((app) => app.id === selectedId) ?? visible[0] ?? null;

  useEffect(() => {
    if (!selected || !engineAvailable()) return;
    let cancelled = false;
    setLeftovers(null);
    setLeftoverError(null);
    engine
      .applicationLeftovers(selected.id)
      .then((rows) => !cancelled && setLeftovers(rows))
      .catch((cause) => !cancelled && setLeftoverError(describeError(cause)));
    return () => {
      cancelled = true;
    };
  }, [selected?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const totalBytes = applications.reduce((sum, app) => sum + app.bundleBytes, 0);
  const counts = {
    all: applications.length,
    unused: applications.filter(
      (app) => app.lastUsedAt !== null && now - app.lastUsedAt > UNUSED_AFTER_SECONDS,
    ).length,
    leftovers: applications.filter((app) => app.leftoverBytes > 0).length,
  };
  const filterLabel = {
    all: "All Applications",
    unused: "Not Opened Lately",
    leftovers: "Keep Files Elsewhere",
  };

  if (!engineAvailable()) {
    return (
      <EmptyState title="The DiskMate engine is not running">
        Start the application with <code className="mono">npm run dev</code>.
      </EmptyState>
    );
  }

  return (
    <div className="apps surface-light">
      <header className="apps__bar">
        <div>
          <button type="button" className="apps__ghost" onClick={onBack}>
            <ChevronLeftIcon size={16} />
            Back
          </button>
        </div>
        <h1 className="apps__title">Applications</h1>
        <div className="apps__tools">
          <label className="apps__sort">
            Sort by:
            <select value={sort} onChange={(event) => setSort(event.target.value as Sort)}>
              <option value="size">Size</option>
              <option value="name">Name</option>
              <option value="opened">Last opened</option>
            </select>
          </label>
          <Button variant="quiet" onClick={loadApplications} disabled={loadingApps}>
            Rescan
          </Button>
        </div>
      </header>

      <div className="apps__body">
        <nav className="apps__filters" aria-label="Application filters">
          {(Object.keys(filterLabel) as Filter[]).map((key) => (
            <button
              key={key}
              type="button"
              aria-current={filter === key ? "true" : undefined}
              onClick={() => setFilter(key)}
            >
              <span>{filterLabel[key]}</span>
              <span className="num faint">{formatCount(counts[key])}</span>
            </button>
          ))}
        </nav>

        <div className="apps__list">
          <div className="apps__list-head">
            <h2>{filterLabel[filter]}</h2>
            <p className="muted">
              {formatCount(applications.length)} apps using {formatBytes(totalBytes)}, plus what
              each one keeps outside its own folder. Apps that come with macOS are not listed.
            </p>
          </div>
          {loadingApps && applications.length === 0 ? (
            <Spinner />
          ) : visible.length === 0 ? (
            <p className="muted" style={{ padding: "var(--space-4)" }}>
              Nothing matches this filter.
            </p>
          ) : (
            visible.slice(0, 120).map((app) => (
              <button
                key={app.id}
                type="button"
                className="apps__row"
                aria-pressed={selected?.id === app.id}
                onClick={() => setSelectedId(app.id)}
              >
                <AppMark name={app.name} />
                <span className="apps__row-text">
                  <span className="apps__row-name">{app.name}</span>
                  <span className="apps__row-meta">
                    {app.lastUsedAt === null
                      ? "Last opened: not recorded"
                      : `Opened ${formatRelative(app.lastUsedAt)}`}
                  </span>
                </span>
                <span className="num apps__row-size">
                  {formatBytes(app.bundleBytes + app.leftoverBytes)}
                </span>
              </button>
            ))
          )}
        </div>

        <div className="apps__detail">
          {cleanup.error && <Note tone="caution">{cleanup.error}</Note>}
          <RemovalReportNote cleanup={cleanup} />
          {selected ? (
            <>
              <div className="apps__detail-head">
                <AppMark name={selected.name} large />
                <h2>{selected.name}</h2>
              </div>
              <dl className="apps__facts">
                <dt>App size</dt>
                <dd className="num">{formatBytes(selected.bundleBytes)}</dd>
                <dt>Kept elsewhere</dt>
                <dd className="num">
                  {selected.leftoverCount > 0
                    ? `${formatBytes(selected.leftoverBytes)} in ${formatCount(selected.leftoverCount)} places`
                    : "Nothing found"}
                </dd>
                <dt>Last opened</dt>
                <dd>
                  {selected.lastUsedAt === null
                    ? "Not recorded"
                    : formatRelative(selected.lastUsedAt)}
                </dd>
                {selected.version && (
                  <>
                    <dt>Version</dt>
                    <dd>{selected.version}</dd>
                  </>
                )}
                {selected.bundleId && (
                  <>
                    <dt>Identifier</dt>
                    <dd className="mono selectable">{selected.bundleId}</dd>
                  </>
                )}
                <dt>Location</dt>
                <dd className="mono selectable" title={selected.path}>
                  {truncatePath(selected.path, 48)}
                </dd>
              </dl>

              <section className="apps__leftovers">
                <h3>What it keeps outside Applications</h3>
                {leftoverError ? (
                  <p className="muted">{leftoverError}</p>
                ) : leftovers === null ? (
                  <Spinner />
                ) : leftovers.length === 0 ? (
                  <p className="muted">DiskMate found nothing for this app in your Library.</p>
                ) : (
                  <ul>
                    {leftovers.map((item) => (
                      <li key={item.path} title={item.reason}>
                        <span className="mono apps__leftover-path">
                          {truncatePath(item.path, 52)}
                        </span>
                        <Safety safety={item.safety} />
                        <span className="num faint">{formatBytes(item.bytes)}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <div className="row">
                <Button onClick={() => void reveal(selected.path)}>Show in Finder</Button>
                <Button onClick={() => cleanup.uninstall(selected.id)}>
                  Add App and Its Files to Queue
                </Button>
              </div>
            </>
          ) : (
            !loadingApps && (
              <EmptyState title="No applications found">
                DiskMate looks in Applications and your home Applications folder.
              </EmptyState>
            )
          )}
        </div>
      </div>

      <footer className="apps__foot">
        Adding an app puts it and its files in the queue. Nothing moves until you approve the
        queue, and then only to the Trash.
      </footer>
    </div>
  );
}

/**
 * A stand-in for the app's icon.
 *
 * Reading real icons needs the platform probe and is not built yet, so this is
 * a neutral tile with the first letter of the name. Its colour comes from the
 * name, so the same app always looks the same.
 */
function AppMark({ name, large }: { name: string; large?: boolean }) {
  const hues = [222, 168, 280, 24, 340, 196, 132, 48];
  let hash = 0;
  for (const char of name) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  const hue = hues[hash % hues.length]!;
  return (
    <span
      className={large ? "app-mark app-mark--large" : "app-mark"}
      style={{
        background: `linear-gradient(150deg, hsl(${hue} 70% 62%), hsl(${hue} 62% 42%))`,
      }}
      aria-hidden="true"
    >
      {name.trim().charAt(0).toUpperCase() || "?"}
    </span>
  );
}
