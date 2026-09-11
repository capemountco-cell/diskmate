import { useEffect, useState } from "react";

import { Badge, EmptyState, Note, Panel, Spinner } from "../../components/primitives";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import type { StartupItem, StartupReport } from "../../lib/types";

/**
 * What runs automatically on this Mac.
 *
 * Read-only, and honest about what it does not know. There is no impact column
 * filled with guesses: until DiskMate can measure launch cost, saying so is the
 * only defensible thing to print.
 */
export function StartupPage() {
  const [report, setReport] = useState<StartupReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!engineAvailable()) {
      setError("The DiskMate engine is not running.");
      return;
    }
    engine
      .startupReport()
      .then(setReport)
      .catch((cause) => setError(describeError(cause)));
  }, []);

  if (error) return <Note tone="caution">{error}</Note>;
  if (!report) return <Spinner />;

  const userItems = report.items.filter((item) => !item.isOsProvided);
  const osItems = report.items.filter((item) => item.isOsProvided);

  return (
    <div className="stack">
      <p className="muted">
        {report.items.length === 0
          ? "DiskMate found no launch items it can read."
          : `${report.items.length} items are configured to start automatically. ` +
            `${osItems.length} of them ship with the operating system.`}
      </p>

      {report.impactUnavailable && (
        <Note tone="info" title="Impact is not measured yet.">
          DiskMate will not guess how much any of these cost at startup. Until it can measure
          that properly, it shows what is configured and leaves the judgement to you.
        </Note>
      )}

      <Note tone="info" title="Read only.">
        This build does not enable, disable or remove launch items. Changing them by hand can
        stop applications working, so DiskMate will only ever do it with your explicit choice.
      </Note>

      <ItemGroup
        title="Installed by apps and by you"
        note="These come from software on this Mac."
        items={userItems}
      />
      <ItemGroup
        title="Part of macOS"
        note="DiskMate will never suggest changing these."
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
  note: string;
  items: StartupItem[];
}) {
  return (
    <Panel title={title} note={note} flush>
      {items.length === 0 ? (
        <EmptyState title="Nothing here" />
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Type</th>
              <th>Runs</th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={`${item.configPath}:${item.id}`}>
                <td>
                  <div className="table__name-text">{item.name}</div>
                  {item.program && (
                    <div className="faint mono table__name-text" title={item.program}>
                      {item.program}
                    </div>
                  )}
                </td>
                <td className="faint">{describeSource(item)}</td>
                <td>{describeRuns(item)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  );
}

function describeSource(item: StartupItem): string {
  switch (item.source) {
    case "launchAgent":
      return item.scope === "user" ? "Launch agent (your account)" : "Launch agent (all users)";
    case "launchDaemon":
      return "Launch daemon (system)";
    case "loginItem":
      return "Login item";
    case "userService":
      return "User service";
    case "systemService":
      return "System service";
    case "autostart":
      return "Autostart entry";
    default:
      return "Unknown";
  }
}

function describeRuns(item: StartupItem) {
  if (item.disabled === true) return <Badge>Disabled</Badge>;
  if (item.runsAtLoad === true) return <Badge tone="accent">At startup</Badge>;
  if (item.runsAtLoad === false) return <Badge>On demand</Badge>;
  return <span className="faint">Not stated</span>;
}
