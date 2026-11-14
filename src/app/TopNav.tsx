import { useEffect, useRef, useState, type CSSProperties } from "react";

import brandMark from "../assets/brand-broom.png";
import { formatBytes } from "../lib/format";
import { engine, engineAvailable } from "../lib/ipc";
import type {
  CandidateGroup,
  MemoryPressure,
  MemoryReport,
  ProcessMemory,
  VolumeInfo,
} from "../lib/types";

export type SectionId = "overview" | "storage" | "cleanup" | "applications" | "memory";

/**
 * Somewhere the app can be sent: a section, or Duplicates, which lives inside
 * Cleanup (opened from its Duplicates row) rather than in the top bar.
 */
export type Destination = SectionId | "duplicates";

/**
 * The sections, each with the colour that marks it as the one you are in.
 *
 * `rgb` and `text` are read by the stylesheet as --item and --item-text, so
 * an item shows its own colour while pressed and once selected. Startup lives
 * inside Apps, as a tab.
 */
export const SECTIONS: Array<{
  id: SectionId;
  label: string;
  panel: boolean;
  rgb: string;
  text: string;
}> = [
  { id: "overview", label: "Overview", panel: false, rgb: "255, 255, 255", text: "#f5f5f7" },
  { id: "storage", label: "Storage", panel: true, rgb: "250, 204, 21", text: "#fde047" },
  { id: "cleanup", label: "Cleanup", panel: true, rgb: "45, 212, 191", text: "#5eead4" },
  { id: "applications", label: "Apps", panel: false, rgb: "129, 140, 248", text: "#a5b4fc" },
  { id: "memory", label: "Memory", panel: true, rgb: "56, 189, 248", text: "#7dd3fc" },
];

/** Sections whose numbers come from a scan, and so update when one finishes. */
const FROM_SCAN: ReadonlySet<SectionId> = new Set(["cleanup"]);

type PanelId = "storage" | "cleanup" | "memory";

/** How long the pointer has to rest on a section before its panel drops. */
const OPEN_DELAY_MS = 120;
/** How long after leaving the bar and panel before the panel closes. */
const CLOSE_DELAY_MS = 220;

/**
 * The global bar.
 *
 * Shaped like Apple's: the mark on the left, the sections centred, the
 * volume on the right. Resting the pointer on Storage, Cleanup or Memory
 * drops a full-width panel of shortcuts and live figures for that section,
 * and the page behind blurs. Clicking a section goes to it. Escape, a click
 * outside, or moving away closes the panel.
 *
 * No item is ever disabled. While a scan runs, Cleanup carries a small white
 * dot (its numbers update when the scan finishes) and still opens to the last
 * results. Memory carries an orange or red dot when pressure is up.
 */
export function TopNav({
  active,
  onSelect,
  volumeName,
  groups,
  stagedCount,
  stagedBytes,
  scanning,
  pressure,
}: {
  active: SectionId;
  onSelect: (id: Destination) => void;
  /** The volume the current scan is of, if there is one. */
  volumeName: string | null;
  groups: CandidateGroup[];
  stagedCount: number;
  stagedBytes: number;
  /** A scan is running, so scan-fed sections are showing the previous one. */
  scanning: boolean;
  pressure: MemoryPressure;
}) {
  const [open, setOpen] = useState<PanelId | null>(null);
  const openTimer = useRef<number | undefined>(undefined);
  const closeTimer = useRef<number | undefined>(undefined);
  // After a click, the pointer is still on the section it clicked; without
  // this the hover that opened the panel would open it again a moment later.
  const suppressed = useRef(false);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && setOpen(null);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  useEffect(
    () => () => {
      window.clearTimeout(openTimer.current);
      window.clearTimeout(closeTimer.current);
    },
    [],
  );

  const hover = (panel: PanelId | null) => {
    window.clearTimeout(closeTimer.current);
    window.clearTimeout(openTimer.current);
    if (panel === null || suppressed.current) return;
    openTimer.current = window.setTimeout(() => setOpen(panel), open ? 0 : OPEN_DELAY_MS);
  };
  const leave = () => {
    suppressed.current = false;
    window.clearTimeout(openTimer.current);
    closeTimer.current = window.setTimeout(() => setOpen(null), CLOSE_DELAY_MS);
  };
  const stay = () => window.clearTimeout(closeTimer.current);

  const go = (id: Destination) => {
    window.clearTimeout(openTimer.current);
    suppressed.current = true;
    setOpen(null);
    onSelect(id);
  };

  return (
    <>
      <header className="globalnav" onMouseLeave={leave} onMouseEnter={stay}>
        <button
          type="button"
          className="brand"
          aria-label="Mr Clean, go to Overview"
          title="Mr Clean"
          onClick={() => go("overview")}
        >
          <img className="brand-mark" src={brandMark} alt="" draggable={false} />
        </button>

        <nav className="places" aria-label="Sections">
          {SECTIONS.map((section) => {
            const waits = scanning && FROM_SCAN.has(section.id);
            const alert =
              section.id === "memory" && (pressure === "warning" || pressure === "critical")
                ? pressure
                : null;
            const label = waits
              ? `${section.label}, updates when the scan finishes`
              : alert
                ? `${section.label}, ${alert === "critical" ? "pressure is high" : "under pressure"}`
                : undefined;
            return (
              <button
                key={section.id}
                type="button"
                style={{ "--item": section.rgb, "--item-text": section.text } as CSSProperties}
                aria-label={label}
                aria-current={section.id === active ? "page" : undefined}
                aria-haspopup={section.panel || undefined}
                aria-expanded={section.panel ? open === section.id : undefined}
                onMouseEnter={() => hover(section.panel ? (section.id as PanelId) : null)}
                onFocus={() => section.panel && open && setOpen(section.id as PanelId)}
                onKeyDown={(event) => {
                  if (section.panel && event.key === "ArrowDown") {
                    event.preventDefault();
                    setOpen(section.id as PanelId);
                  }
                }}
                onClick={() => go(section.id)}
              >
                {section.label}
                {waits && <span className="places__dot" aria-hidden="true" />}
                {alert && (
                  <span className={`places__dot places__dot--${alert}`} aria-hidden="true" />
                )}
              </button>
            );
          })}
        </nav>

        <button
          type="button"
          className={scanning ? "volume-pill volume-pill--working" : "volume-pill"}
          aria-haspopup
          aria-expanded={open === "storage"}
          onClick={() => setOpen(open === "storage" ? null : "storage")}
        >
          {scanning ? (
            <span className="volume-pill__dot" aria-hidden="true" />
          ) : (
            <svg
              width="15"
              height="15"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.7"
              aria-hidden="true"
            >
              <rect x="2" y="4.5" width="12" height="7" rx="1.6" />
              <path d="M5 8h.01" />
            </svg>
          )}
          {scanning ? `Reading ${volumeName ?? "a volume"}` : (volumeName ?? "Choose a volume")}
          <svg
            width="11"
            height="11"
            viewBox="0 0 12 12"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.2"
            aria-hidden="true"
          >
            <path d={open === "storage" ? "m3 7.5 3-3 3 3" : "m3 4.5 3 3 3-3"} />
          </svg>
        </button>
      </header>

      {open && (
        <>
          <button
            type="button"
            className="flyout-scrim"
            aria-label="Close menu"
            onClick={() => setOpen(null)}
          />
          <section
            className="flyout"
            aria-label={`${open} menu`}
            onMouseEnter={stay}
            onMouseLeave={leave}
          >
            {open === "storage" && <StorageFlyout go={go} />}
            {open === "cleanup" && (
              <CleanupFlyout
                go={go}
                groups={groups}
                stagedCount={stagedCount}
                stagedBytes={stagedBytes}
              />
            )}
            {open === "memory" && <MemoryFlyout go={go} />}
          </section>
        </>
      )}
    </>
  );
}

function StorageFlyout({ go }: { go: (id: Destination) => void }) {
  const [volumes, setVolumes] = useState<VolumeInfo[] | null>(null);
  useEffect(() => {
    if (!engineAvailable()) return setVolumes([]);
    engine
      .listVolumes()
      .then(setVolumes)
      .catch(() => setVolumes([]));
  }, []);

  return (
    <>
      <div>
        <p className="flyout__label">Explore storage</p>
        <button type="button" className="flyout__big" onClick={() => go("storage")}>
          Scan a volume
        </button>
        <button type="button" className="flyout__big" onClick={() => go("storage")}>
          Map of every folder
        </button>
        <button type="button" className="flyout__big" onClick={() => go("storage")}>
          Largest files
        </button>
        <button type="button" className="flyout__big" onClick={() => go("storage")}>
          What changed
        </button>
      </div>
      <div>
        <p className="flyout__label">Volumes</p>
        {volumes === null ? (
          <p className="flyout__note">Reading volumes…</p>
        ) : volumes.length === 0 ? (
          <p className="flyout__note">No volumes could be read.</p>
        ) : (
          volumes.slice(0, 4).map((volume) => {
            const share =
              volume.usedBytes !== null && volume.totalBytes
                ? volume.usedBytes / volume.totalBytes
                : null;
            return (
              <div key={volume.mountPoint} className="flyout__volume">
                <div className="flyout__row">
                  <span className="flyout__name">{volume.name}</span>
                  <span className="flyout__meta">
                    {volume.usedBytes !== null && volume.totalBytes
                      ? `${formatBytes(volume.usedBytes)} of ${formatBytes(volume.totalBytes)}`
                      : "Capacity unknown"}
                  </span>
                </div>
                {share !== null && (
                  <div className="flyout__bar" aria-hidden="true">
                    <i style={{ width: `${share * 100}%` }} />
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
      <div>
        <p className="flyout__label">Not seeing a volume?</p>
        <p className="flyout__note">
          Plug it in and unlock it. To scan the whole startup disk, turn on Full Disk Access for
          Mr Clean in System Settings › Privacy &amp; Security, or choose a single folder from
          Storage.
        </p>
      </div>
    </>
  );
}

function CleanupFlyout({
  go,
  groups,
  stagedCount,
  stagedBytes,
}: {
  go: (id: Destination) => void;
  groups: CandidateGroup[];
  stagedCount: number;
  stagedBytes: number;
}) {
  const top = [...groups].sort((a, b) => b.bytes - a.bytes).slice(0, 4);
  return (
    <>
      <div>
        <p className="flyout__label">Cleanup</p>
        <button type="button" className="flyout__big" onClick={() => go("cleanup")}>
          Review findings
        </button>
        <button type="button" className="flyout__big" onClick={() => go("duplicates")}>
          Duplicates
        </button>
        <button type="button" className="flyout__big" onClick={() => go("applications")}>
          Apps and leftovers
        </button>
      </div>
      <div>
        <p className="flyout__label">Found</p>
        {top.length === 0 ? (
          <p className="flyout__note">Nothing yet. Findings come from a scan.</p>
        ) : (
          top.map((group) => (
            <div
              key={`${group.kind}-${group.safety}`}
              className="flyout__row flyout__row--line"
            >
              <span className="flyout__name">{group.label}</span>
              <span
                className={
                  group.kind === "duplicateCopy" ? "tone-copies num" : "tone-reclaim num"
                }
              >
                {formatBytes(group.bytes)}
              </span>
            </div>
          ))
        )}
      </div>
      <div>
        <p className="flyout__label">Your queue</p>
        <p className="flyout__name">
          {stagedCount === 0
            ? "Nothing yet."
            : `${stagedCount} items · ${formatBytes(stagedBytes)}`}
        </p>
        <p className="flyout__note">
          Nothing moves until you approve the queue, and then only to the Trash.
        </p>
      </div>
    </>
  );
}

function MemoryFlyout({ go }: { go: (id: Destination) => void }) {
  const [report, setReport] = useState<MemoryReport | null>(null);
  const [apps, setApps] = useState<ProcessMemory[] | null>(null);
  useEffect(() => {
    if (!engineAvailable()) return;
    engine
      .memoryReport()
      .then(setReport)
      .catch(() => setReport(null));
    engine
      .topApps(3)
      .then(setApps)
      .catch(() => setApps([]));
  }, []);

  const pressure = report?.snapshot.pressure ?? "unknown";
  return (
    <>
      <div>
        <p className="flyout__label">Memory</p>
        <button type="button" className="flyout__big" onClick={() => go("memory")}>
          Watch memory
        </button>
        <button type="button" className="flyout__big" onClick={() => go("memory")}>
          Apps using the most
        </button>
      </div>
      <div>
        <p className="flyout__label">Right now</p>
        {report ? (
          <>
            <p className="flyout__figure">
              {formatBytes(report.snapshot.usedBytes)}{" "}
              <span>of {formatBytes(report.snapshot.totalBytes)}</span>
            </p>
            <p className="flyout__note">
              <i className={`state-dot state-dot--${pressure}`} />
              {report.headline}
            </p>
          </>
        ) : (
          <p className="flyout__note">Reading memory…</p>
        )}
      </div>
      <div>
        <p className="flyout__label">Using the most</p>
        {apps === null ? (
          <p className="flyout__note">Reading…</p>
        ) : (
          apps.map((app) => (
            <div key={`${app.pid}-${app.name}`} className="flyout__row flyout__row--line">
              <span className="flyout__name">{app.name}</span>
              <span className="flyout__meta num">{formatBytes(app.residentBytes)}</span>
            </div>
          ))
        )}
        <p className="flyout__note">Mr Clean also shows memory in the menu bar.</p>
      </div>
    </>
  );
}
