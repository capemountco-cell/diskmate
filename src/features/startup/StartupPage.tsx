import { useEffect, useState } from "react";

import { Button, Note, Spinner } from "../../components/primitives";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import type { StartupItem, StartupReport } from "../../lib/types";

/**
 * What runs automatically on this Mac: the Startup tab inside Apps.
 *
 * Read-only, and honest about what it does not know. There is no switch and no
 * impact column: Mr Clean does not change launch items (macOS's own Login
 * Items settings do that), and it will not guess what one costs at startup.
 */
export function StartupTab({
  report,
  onReport,
}: {
  /** Held by the shell so leaving and returning does not read everything again. */
  report: StartupReport | null;
  onReport: (report: StartupReport) => void;
}) {
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const look = () => {
    if (!engineAvailable()) {
      setError("The Mr Clean engine is not running.");
      return;
    }
    setLoading(true);
    setError(null);
    engine
      .startupReport()
      .then(onReport)
      .catch((cause) => setError(describeError(cause)))
      .finally(() => setLoading(false));
  };

  // Reading launch settings is quick and changes nothing, so the tab reads
  // them on first open rather than asking.
  useEffect(() => {
    if (!report) look();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (error) return <Note tone="caution">{error}</Note>;
  if (!report) return <Spinner />;

  const userItems = report.items.filter((item) => !item.isOsProvided);
  const osItems = report.items.filter((item) => item.isOsProvided);

  return (
    <div className="stack">
      <div className="tab-intro">
        <p className="lede">
          {report.items.length === 0 ? (
            "Mr Clean found no launch items it can read."
          ) : (
            <>
              <strong className="num">{userItems.length}</strong>{" "}
              {userItems.length === 1 ? "item starts" : "items start"} with this Mac, put there
              by apps or by you. To turn one off, use Login Items in System Settings; Mr Clean
              only reads them.
            </>
          )}
        </p>
        <Button variant="quiet" onClick={look} disabled={loading}>
          {loading ? "Reading…" : "Read again"}
        </Button>
      </div>

      <ItemGroup title="Installed by apps and by you" items={userItems} />
      <ItemGroup
        title={`Part of macOS · ${osItems.length}`}
        note="Mr Clean will never suggest changing these."
        items={osItems}
      />
    </div>
  );
}

function ItemGroup({
  title,
  note,
  items,
}: {
  title: string;
  note?: string;
  items: StartupItem[];
}) {
  return (
    <section className="group">
      <div className="group__head">
        <p className="group__title">{title}</p>
        {note && <p className="group__note">{note}</p>}
      </div>
      <div className="rows">
        {items.length === 0 ? (
          <p className="rows__empty">Nothing here.</p>
        ) : (
          items.map((item) => (
            <div key={`${item.configPath}:${item.id}`} className="row-item row-item--compact">
              <span className="app-tile" aria-hidden="true">
                {item.name.trim().charAt(0).toUpperCase() || "?"}
              </span>
              <div className="row-item__text">
                <p className="row-item__title">{item.name}</p>
                <p className="row-item__detail mono" title={item.program ?? item.configPath}>
                  {describeSource(item)}
                  {item.program ? ` · ${item.program}` : ""}
                </p>
              </div>
              <span className="row-item__aside">{describeRuns(item)}</span>
            </div>
          ))
        )}
      </div>
    </section>
  );
}

function describeSource(item: StartupItem): string {
  switch (item.source) {
    case "launchAgent":
      return item.scope === "user" ? "Launch agent, your account" : "Launch agent, all users";
    case "launchDaemon":
      return "Launch daemon, system";
    case "loginItem":
      return "Login item";
    case "userService":
      return "User service";
    case "systemService":
      return "System service";
    case "autostart":
      return "Autostart entry";
    default:
      return "Unknown kind";
  }
}

function describeRuns(item: StartupItem): string {
  if (item.disabled === true) return "Turned off";
  if (item.runsAtLoad === true) return "At login";
  if (item.runsAtLoad === false) return "When needed";
  return "Not stated";
}
