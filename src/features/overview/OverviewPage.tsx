import { useEffect, useState, type ReactNode } from "react";

import {
  ApplicationsIcon,
  CleanupIcon,
  DuplicatesIcon,
  MemoryIcon,
  StartupIcon,
} from "../../components/icons";
import { Button, EmptyState } from "../../components/primitives";
import { formatBytes, formatCount, formatRelative } from "../../lib/format";
import { engine, engineAvailable } from "../../lib/ipc";
import type {
  MemoryPressure,
  MemoryReport,
  ScanSummary,
  StartupReport,
  VolumeInfo,
} from "../../lib/types";
import type { SectionId } from "../../app/TopNav";
import type { CleanupController } from "../cleanup/useCleanup";

/** How often the memory row is refreshed while Overview is open. */
const MEMORY_POLL_MS = 5_000;

/**
 * Overview.
 *
 * One headline (the disk the last scan read) and one list, a row per section,
 * in the order you would work through them. Every number is white: a size is a
 * size whatever section it belongs to. Colour appears only for something true,
 * always with a word: green for freed or healthy, orange for worth a look, red
 * for act now. A row with nothing measured behind it says so and offers the
 * button that would measure it.
 */
export function OverviewPage({
  scan,
  scanning,
  cleanup,
  startup,
  onPressure,
  onGo,
  onGoToStartup,
  onWipe,
}: {
  scan: ScanSummary | null;
  scanning: boolean;
  cleanup: CleanupController;
  startup: StartupReport | null;
  onPressure: (pressure: MemoryPressure) => void;
  onGo: (section: SectionId) => void;
  onGoToStartup: () => void;
  onWipe: () => void;
}) {
  const [volumes, setVolumes] = useState<VolumeInfo[] | null>(null);
  const [memory, setMemory] = useState<MemoryReport | null>(null);

  useEffect(() => {
    if (!engineAvailable()) return;
    engine
      .listVolumes()
      .then(setVolumes)
      .catch(() => setVolumes([]));
    const readMemory = () =>
      engine
        .memoryReport()
        .then((report) => {
          setMemory(report);
          onPressure(report.snapshot.pressure);
        })
        .catch(() => setMemory(null));
    void readMemory();
    const timer = window.setInterval(readMemory, MEMORY_POLL_MS);
    return () => window.clearInterval(timer);
  }, [onPressure]);

  if (!engineAvailable()) {
    return (
      <EmptyState title="The Mr Clean engine is not running">
        Start the application with <code className="mono">npm run dev</code>.
      </EmptyState>
    );
  }

  const list = volumes ?? [];
  const volume =
    (scan && list.find((candidate) => candidate.mountPoint === scan.rootPath)) ??
    list.find((candidate) => candidate.mountPoint === "/") ??
    list.find((candidate) => candidate.kind === "internal") ??
    list[0] ??
    null;
  const capacity =
    volume && volume.usedBytes !== null && volume.totalBytes
      ? {
          used: volume.usedBytes,
          total: volume.totalBytes,
          share: volume.usedBytes / volume.totalBytes,
        }
      : null;
  const fullness: Status | null = !capacity
    ? null
    : capacity.share >= 0.95
      ? { tone: "act", word: "Almost full" }
      : capacity.share >= 0.85
        ? { tone: "attention", word: "Getting full" }
        : null;

  const reclaimGroups = cleanup.groups.filter((group) => group.kind !== "duplicateCopy");
  const copyGroups = cleanup.groups.filter((group) => group.kind === "duplicateCopy");
  const reclaimBytes = reclaimGroups.reduce((sum, group) => sum + group.bytes, 0);
  const copyBytes = copyGroups.reduce((sum, group) => sum + group.bytes, 0);
  const copyCount = copyGroups.reduce((sum, group) => sum + group.count, 0);
  const freed = cleanup.report && cleanup.report.removedBytes > 0 ? cleanup.report : null;

  const apps = cleanup.applications;
  const appBytes = apps.reduce((sum, app) => sum + app.bundleBytes, 0);
  const userStartup = startup ? startup.items.filter((item) => !item.isOsProvided) : [];

  const pressure = memory?.snapshot.pressure ?? "unknown";
  const memoryStatus: Status | null =
    pressure === "normal"
      ? { tone: "good", word: "Normal", quiet: true }
      : pressure === "warning"
        ? { tone: "attention", word: "Under pressure" }
        : pressure === "critical"
          ? { tone: "act", word: "High" }
          : null;

  const kicker = scan
    ? scanning
      ? `Showing the scan from ${formatRelative(scan.startedAt)}. A new one is running.`
      : `${scanTarget(scan, volume)}, scanned ${formatRelative(scan.startedAt)}`
    : "Nothing scanned yet";

  return (
    <div className="page ov">
      <header className="ov__head">
        <p className="kicker">
          {kicker}
          {fullness && <StatusChip status={fullness} />}
        </p>
        {capacity ? (
          <>
            <p className="bigline">
              <b className="num">{formatBytes(capacity.used)}</b>
              <span>of {formatBytes(capacity.total)} used</span>
            </p>
            <div
              className="ov__bar"
              role="img"
              aria-label={`${Math.round(capacity.share * 100)} percent of ${volume?.name ?? "the disk"} is used`}
            >
              <i
                className={fullness ? `ov__bar-fill--${fullness.tone}` : undefined}
                style={{ width: `${Math.min(capacity.share, 1) * 100}%` }}
              />
            </div>
          </>
        ) : (
          <p className="bigline">
            <span>{volumes === null ? "Reading volumes…" : "Capacity unknown"}</span>
          </p>
        )}
        <div className="ov__lede-row">
          <p className="lede">
            {capacity && `${formatBytes(capacity.total - capacity.used)} free. `}
            {reclaimBytes + copyBytes > 0 ? (
              <>
                <strong className="num">{formatBytes(reclaimBytes + copyBytes)}</strong> could
                be removed, after you look.
              </>
            ) : scan ? (
              "Cleanup has nothing to suggest from the last scan."
            ) : (
              "Scan a volume and Mr Clean will explain where the space went."
            )}
          </p>
          <Button onClick={() => onGo("storage")}>
            {scan ? "Scan again" : "Scan a volume"}
          </Button>
        </div>
      </header>

      <section className="rows" aria-label="Sections">
        <Row
          icon={<CleanupIcon size={20} />}
          title="Cleanup"
          detail={
            reclaimGroups.length > 0
              ? `${sentenceCase(
                  reclaimGroups
                    .map((group) => group.label.toLowerCase())
                    .slice(0, 3)
                    .join(", "),
                )}. Each one explained.`
              : scan
                ? "Caches, build output, old downloads"
                : "Works from a scan"
          }
          status={
            freed ? { tone: "good", word: `Freed ${formatBytes(freed.removedBytes)}` } : null
          }
          value={reclaimGroups.length > 0 ? formatBytes(reclaimBytes) : scan ? "Nothing" : "—"}
          sub={
            reclaimGroups.length > 0
              ? `${formatCount(reclaimGroups.length)} ${reclaimGroups.length === 1 ? "group" : "groups"}`
              : scan
                ? "worth suggesting"
                : "needs a scan"
          }
          action={scan ? "Open" : "Scan"}
          onOpen={() => onGo(scan ? "cleanup" : "storage")}
        />
        <Row
          icon={<DuplicatesIcon size={20} />}
          title="Extra copies"
          detail="Identical contents in more than one place"
          value={copyGroups.length > 0 ? formatBytes(copyBytes) : "—"}
          sub={
            copyGroups.length > 0
              ? `${formatCount(copyCount)} ${copyCount === 1 ? "copy" : "copies"}`
              : "not compared yet"
          }
          action="Open"
          onOpen={() => onGo("duplicates")}
        />
        <Row
          icon={<ApplicationsIcon size={20} />}
          title="Apps"
          detail="Plus what each one keeps outside its own folder"
          value={apps.length > 0 ? formatBytes(appBytes) : "—"}
          sub={apps.length > 0 ? `${formatCount(apps.length)} apps` : "not measured yet"}
          action="Open"
          onOpen={() => onGo("applications")}
        />
        <Row
          icon={<StartupIcon size={20} />}
          title="Starts with your Mac"
          detail="Login items and background items"
          value={startup ? formatCount(userStartup.length) : "—"}
          sub={startup ? (userStartup.length === 1 ? "item" : "items") : "not looked yet"}
          action="Open"
          onOpen={onGoToStartup}
        />
        <Row
          icon={<MemoryIcon size={20} />}
          title="Memory now"
          detail={
            memory
              ? memory.snapshot.swapUsedBytes === null
                ? memory.headline
                : memory.snapshot.swapUsedBytes === 0
                  ? "Nothing moved to disk"
                  : `${formatBytes(memory.snapshot.swapUsedBytes)} moved to disk (swap)`
              : "Reading…"
          }
          status={memoryStatus}
          tone={pressure === "critical" ? "act" : undefined}
          value={memory ? formatBytes(memory.snapshot.usedBytes) : "—"}
          sub={memory ? `of ${formatBytes(memory.snapshot.totalBytes)}` : ""}
          action={pressure === "warning" || pressure === "critical" ? "See heavy apps" : "Open"}
          onOpen={() => onGo("memory")}
        />
      </section>

      <section className="rows" aria-label="Wipe down this Mac">
        <div className="row-item">
          <span className="row-item__tile" aria-hidden="true">
            <svg
              width="20"
              height="20"
              viewBox="0 0 20 20"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <rect x="2.5" y="5" width="15" height="9" rx="1.5" />
              <path d="M5.5 8h.01M8.5 8h.01M11.5 8h.01M14.5 8h.01M6.5 11h7" />
            </svg>
          </span>
          <div className="row-item__text">
            <p className="row-item__title">Wipe down this Mac</p>
            <p className="row-item__detail">
              Locks the keyboard and trackpad and turns the screen black while you clean them.
            </p>
          </div>
          <Button onClick={onWipe}>Start…</Button>
        </div>
      </section>

      <footer className="ov__foot">
        <span>Everything Mr Clean learns stays on this Mac.</span>
        <Button onClick={() => onGo("storage")}>Open Storage</Button>
      </footer>
    </div>
  );
}

type Tone = "good" | "attention" | "act";
type Status = { tone: Tone; word: string; quiet?: boolean };

/** A status: a dot and a word. Quiet ones (healthy) keep the word grey. */
export function StatusChip({ status }: { status: Status }) {
  return (
    <span
      className={[
        "status-chip",
        `status-chip--${status.tone}`,
        status.quiet ? "status-chip--quiet" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <i aria-hidden="true" />
      {status.word}
    </span>
  );
}

function Row({
  icon,
  title,
  detail,
  status,
  tone,
  value,
  sub,
  action,
  onOpen,
}: {
  icon: ReactNode;
  title: string;
  detail: string;
  status?: Status | null;
  /** Tints the whole row; only for "act now". */
  tone?: "act";
  value: string;
  sub: string;
  action: string;
  onOpen: () => void;
}) {
  return (
    <div className={tone ? `row-item row-item--${tone}` : "row-item"}>
      <span className="row-item__tile" aria-hidden="true">
        {icon}
      </span>
      <div className="row-item__text">
        <p className="row-item__title">{title}</p>
        <p className="row-item__detail">{detail}</p>
      </div>
      <div className="row-item__status">{status && <StatusChip status={status} />}</div>
      <div className="row-item__value">
        <p className="num">{value}</p>
        <p>{sub}</p>
      </div>
      <Button onClick={onOpen} aria-label={`${action}: ${title}`}>
        {action}
      </Button>
    </div>
  );
}

function scanTarget(scan: ScanSummary, volume: VolumeInfo | null): string {
  if (volume && volume.mountPoint === scan.rootPath) return volume.name;
  if (scan.rootPath === "/") return "The startup disk";
  return scan.rootPath.replace(/\/+$/, "").split("/").pop() ?? scan.rootPath;
}

function sentenceCase(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
