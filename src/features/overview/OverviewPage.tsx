import { useEffect, useState } from "react";

import { DisplayTitle, MetricCard } from "../../components/layout";
import { Button, EmptyState } from "../../components/primitives";
import { formatBytes, formatCount, formatDuration, formatRelative } from "../../lib/format";
import { engine, engineAvailable } from "../../lib/ipc";
import type {
  AppInfo,
  CandidateGroup,
  MemoryReport,
  ScanSummary,
  StartupReport,
  VolumeInfo,
} from "../../lib/types";

/**
 * The findings bar. Teal means space you can get back, so every group is a
 * shade of it, told apart by lightness; extra copies keep their violet.
 */
const RECLAIM_SHADES = ["#99f6e4", "#5eead4", "#2dd4bf", "#14b8a6", "#0d9488", "#0f766e"];
const COPIES = "#c4b5fd";
function findingColor(group: CandidateGroup, index: number): string {
  return group.kind === "duplicateCopy"
    ? COPIES
    : RECLAIM_SHADES[index % RECLAIM_SHADES.length]!;
}

/**
 * This Mac.
 *
 * A summary of what the other sections have measured, and nothing else. There
 * is no health grade and no "time saved": a grade would be an opinion dressed
 * as a measurement, and Mr Clean has no way to know what anyone's time is
 * worth. A card with nothing measured behind it says so and offers the button
 * that would measure it.
 */
export function OverviewPage({
  info,
  scan,
  groups,
  startup,
  onGo,
}: {
  info: AppInfo | null;
  scan: ScanSummary | null;
  groups: CandidateGroup[];
  startup: StartupReport | null;
  onGo: (section: "storage" | "cleanup" | "startup" | "memory") => void;
}) {
  const [volumes, setVolumes] = useState<VolumeInfo[]>([]);
  const [memory, setMemory] = useState<MemoryReport | null>(null);

  useEffect(() => {
    if (!engineAvailable()) return;
    engine
      .listVolumes()
      .then(setVolumes)
      .catch(() => setVolumes([]));
    engine
      .memoryReport()
      .then(setMemory)
      .catch(() => setMemory(null));
  }, []);

  if (!engineAvailable()) {
    return (
      <EmptyState title="The Mr Clean engine is not running">
        Start the application with <code className="mono">npm run dev</code>.
      </EmptyState>
    );
  }

  const boot =
    volumes.find((volume) => volume.mountPoint === "/") ??
    volumes.find((volume) => volume.kind === "internal") ??
    volumes[0] ??
    null;
  const share =
    boot && boot.usedBytes !== null && boot.totalBytes
      ? boot.usedBytes / boot.totalBytes
      : null;

  const findingsTotal = groups.reduce((sum, group) => sum + group.bytes, 0);
  const topGroups = [...groups].sort((a, b) => b.bytes - a.bytes).slice(0, 4);
  const largest = topGroups[0] ?? null;
  const userStartup = startup ? startup.items.filter((item) => !item.isOsProvided) : [];

  return (
    <div className="page page--dashboard">
      <DisplayTitle
        title="This Mac"
        aside={scan ? `Last scanned ${formatRelative(scan.startedAt)}` : "Not scanned yet"}
      />

      <section className="dashboard">
        <article className="metric metric--hero">
          <span className="metric__glow" aria-hidden="true" />
          <span className="laptop" aria-hidden="true">
            <span className="laptop__screen" />
            <span className="laptop__base" />
          </span>
          <div className="metric__copy">
            <div className="metric__label">This Mac</div>
            {info && <div className="metric__measured">{describeSystem(info)}</div>}
          </div>
          <div className="metric__spacer" />
          {boot ? (
            <div className="capacity">
              <div className="capacity__row">
                <span className="capacity__name">{boot.name}</span>
                <span className="num capacity__used">
                  {boot.usedBytes !== null && boot.totalBytes
                    ? `${formatBytes(boot.usedBytes)} of ${formatBytes(boot.totalBytes)} used`
                    : "Capacity unknown"}
                </span>
              </div>
              {share !== null && (
                <div
                  className="capacity__track"
                  role="img"
                  aria-label={`${Math.round(share * 100)} percent used`}
                >
                  <div className="capacity__fill" style={{ width: `${share * 100}%` }} />
                </div>
              )}
            </div>
          ) : (
            <div className="metric__measured">Reading volumes…</div>
          )}
        </article>

        <MetricCard
          label="Cleanup Can Explain"
          value={scan && groups.length > 0 ? formatBytes(findingsTotal) : "Not looked yet"}
          measured={
            scan
              ? groups.length > 0
                ? `From the scan ${formatRelative(scan.startedAt)}`
                : "Open Cleanup to look"
              : "Needs a scan first"
          }
        >
          {topGroups.length > 0 ? (
            <>
              <div
                className="segbar"
                role="img"
                aria-label={topGroups
                  .map((group) => `${group.label} ${formatBytes(group.bytes)}`)
                  .join(", ")}
              >
                {topGroups.map((group, index) => (
                  <i
                    key={`${group.kind}-${group.safety}`}
                    style={{
                      flexGrow: group.bytes,
                      background: findingColor(group, index),
                    }}
                  />
                ))}
              </div>
              <ul className="metric__rows">
                {topGroups.map((group, index) => (
                  <li key={`${group.kind}-${group.safety}`}>
                    <span>
                      <i className="dot" style={{ background: findingColor(group, index) }} />
                      {group.label}
                    </span>
                    <b className="num">{formatBytes(group.bytes)}</b>
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <div className="metric__foot">
              <Button onClick={() => onGo(scan ? "cleanup" : "storage")}>
                {scan ? "Open Cleanup" : "Scan a Volume"}
              </Button>
            </div>
          )}
        </MetricCard>

        <MetricCard
          label="Last Scan"
          value={scan ? formatDuration(scan.durationMs) : "None yet"}
          measured={
            scan
              ? `to read ${scan.rootPath === "/" ? "the startup disk" : scan.rootPath}`
              : "Storage has not scanned anything"
          }
        >
          {scan ? (
            <ul className="metric__rows">
              <li>
                <span>Files read</span>
                <b className="num">{formatCount(scan.fileCount)}</b>
              </li>
              <li>
                <span>Folders read</span>
                <b className="num">{formatCount(scan.dirCount)}</b>
              </li>
              <li>
                <span>Could not read</span>
                <b className="num">{formatCount(scan.errorCount)}</b>
              </li>
            </ul>
          ) : (
            <div className="metric__foot">
              <Button onClick={() => onGo("storage")}>Go to Storage</Button>
            </div>
          )}
        </MetricCard>

        <article className="metric metric--tip">
          {largest ? (
            <>
              <div className="metric__measured">Where to start</div>
              <div className="metric__tip-title">
                {largest.label} {largest.label.endsWith("s") ? "are" : "is"} the largest group
              </div>
              <p className="metric__tip-body">
                {formatBytes(largest.bytes)} in {formatCount(largest.count)} items.{" "}
                {largest.summary}
              </p>
              <div className="metric__foot">
                <Button onClick={() => onGo("cleanup")}>Open Cleanup</Button>
              </div>
            </>
          ) : (
            <>
              <div className="metric__measured">Where to start</div>
              <div className="metric__tip-title">Scan a volume</div>
              <p className="metric__tip-body">
                Everything on this page that is about storage comes from a scan. Until there is
                one, Mr Clean has nothing to point at.
              </p>
              <div className="metric__foot">
                <Button onClick={() => onGo("storage")}>Go to Storage</Button>
              </div>
            </>
          )}
        </article>

        <MetricCard
          label="Startup"
          value={startup ? `${formatCount(userStartup.length)} items` : "Not looked yet"}
          measured={
            startup
              ? `installed by apps or by you · ${formatCount(startup.osCount)} more from macOS`
              : "Reads launch settings only"
          }
        >
          {!startup && (
            <div className="metric__foot">
              <Button onClick={() => onGo("startup")}>Open Startup</Button>
            </div>
          )}
        </MetricCard>

        <MetricCard
          label="Memory Right Now"
          value={memory ? formatBytes(memory.snapshot.usedBytes) : "Reading…"}
          unit={memory ? `of ${formatBytes(memory.snapshot.totalBytes)}` : undefined}
          measured={memory ? memory.headline : undefined}
        >
          <div className="metric__foot">
            <Button onClick={() => onGo("memory")}>Watch Memory</Button>
          </div>
        </MetricCard>
      </section>
    </div>
  );
}

function describeSystem(info: AppInfo): string {
  const os = info.os === "macos" ? "macOS" : info.os;
  const chip =
    info.os === "macos" && info.arch === "aarch64"
      ? "Apple silicon"
      : info.os === "macos" && info.arch === "x86_64"
        ? "Intel"
        : info.arch;
  return `${os} · ${chip} · Mr Clean ${info.version}`;
}
