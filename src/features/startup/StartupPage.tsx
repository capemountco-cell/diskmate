import { useState } from "react";

import { ClockIcon, LeftBehindIcon, StartupIcon } from "../../components/icons";
import { Hero, IntroScreen, PageHeader } from "../../components/layout";
import { Badge, EmptyState, Note, Panel, Spinner } from "../../components/primitives";
import { describeError, engine, engineAvailable } from "../../lib/ipc";
import type { StartupItem, StartupReport } from "../../lib/types";

/**
 * What runs automatically on this Mac.
 *
 * Read-only, and honest about what it does not know. There is no impact column
 * filled with guesses: until Mr Clean can measure launch cost, saying so is the
 * only defensible thing to print.
 */
export function StartupPage({
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

  if (!report) {
    return (
      <>
        <IntroScreen
          title="Startup"
          subtitle="See everything that starts with your Mac, who put it there, and where it lives."
          art={
            <div className="device-art" aria-hidden="true">
              <StartupIcon size={72} />
            </div>
          }
          features={[
            { icon: <StartupIcon size={20} />, label: "Login Items" },
            { icon: <ClockIcon size={20} />, label: "Background Items" },
            { icon: <LeftBehindIcon size={20} />, label: "Part of macOS, Kept Apart" },
          ]}
          action={{ label: loading ? "Reading…" : "Scan", onClick: look, disabled: loading }}
          footnote="Reads launch settings only. Changes nothing."
        />
        {error && (
          <div className="intro__error">
            <Note tone="caution">{error}</Note>
          </div>
        )}
      </>
    );
  }

  const userItems = report.items.filter((item) => !item.isOsProvided);
  const osItems = report.items.filter((item) => item.isOsProvided);

  return (
    <div className="page">
      <PageHeader title="Startup" back={{ label: "Look again", onClick: look }} />
      {loading && <Spinner />}
      <Hero
        title={
          report.items.length === 0 ? (
            "Mr Clean found no launch items it can read."
          ) : (
            <>
              <span className="num">{userItems.length}</span>{" "}
              {userItems.length === 1 ? "thing starts" : "things start"} with this Mac.
            </>
          )
        }
        subtitle={
          report.items.length === 0
            ? undefined
            : `Installed by apps or by you. Another ${osItems.length} ${osItems.length === 1 ? "ships" : "ship"} with macOS and ${osItems.length === 1 ? "is" : "are"} listed separately.`
        }
      />

      <div className="stack">
        {report.impactUnavailable && (
          <Note tone="info" title="Impact is not measured yet.">
            Mr Clean will not guess how much any of these cost at startup. Until it can measure
            that properly, it shows what is configured and leaves the judgement to you.
          </Note>
        )}

        <Note tone="info" title="Read only.">
          This build does not enable, disable or remove launch items. To turn one off, use Login
          Items in System Settings, which keeps macOS in charge of the change.
        </Note>

        <ItemGroup
          title="Installed by apps and by you"
          note="These come from software on this Mac."
          items={userItems}
        />
        <ItemGroup
          title="Part of macOS"
          note="Mr Clean will never suggest changing these."
          items={osItems}
        />
      </div>
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
