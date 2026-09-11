import { Meter } from "../../components/Meter";
import { Button, EmptyState, Note, Panel } from "../../components/primitives";
import { formatBytes, formatPercent } from "../../lib/format";
import type { VolumeInfo } from "../../lib/types";

export function VolumePicker({
  volumes,
  selectedRoot,
  loading,
  error,
  onSelect,
  onRefresh,
  onChooseFolder,
}: {
  volumes: VolumeInfo[];
  selectedRoot: string | null;
  loading: boolean;
  error: string | null;
  onSelect: (mountPoint: string) => void;
  onRefresh: () => void;
  onChooseFolder: () => void;
}) {
  const folderSelected = selectedRoot !== null && !volumes.some((v) => v.mountPoint === selectedRoot);

  return (
    <Panel
      title="Volumes"
      action={
        <span className="row">
          <Button variant="quiet" onClick={onChooseFolder}>
            Choose a folder…
          </Button>
          <Button variant="quiet" onClick={onRefresh} disabled={loading}>
            Refresh
          </Button>
        </span>
      }
    >
      {error && <Note tone="caution">{error}</Note>}

      {!error && loading && volumes.length === 0 && <p className="muted">Looking for volumes…</p>}

      {!error && !loading && volumes.length === 0 && (
        <EmptyState title="No volumes found">
          DiskMate could not read the list of mounted volumes on this system.
        </EmptyState>
      )}

      <div className="volume-list">
        {volumes.map((volume) => (
          <VolumeOption
            key={volume.mountPoint}
            volume={volume}
            selected={volume.mountPoint === selectedRoot}
            onSelect={() => onSelect(volume.mountPoint)}
          />
        ))}

        {folderSelected && (
          <button type="button" className="volume-option" aria-pressed onClick={() => {}}>
            <span className="volume-option__name">{folderName(selectedRoot!)}</span>
            <span className="volume-option__capacity">Folder</span>
            <span className="volume-option__meta">{selectedRoot}</span>
          </button>
        )}
      </div>
    </Panel>
  );
}

function VolumeOption({
  volume,
  selected,
  onSelect,
}: {
  volume: VolumeInfo;
  selected: boolean;
  onSelect: () => void;
}) {
  const used = volume.usedBytes;
  const total = volume.totalBytes;
  const share = used !== null && total ? used / total : null;

  return (
    <button type="button" className="volume-option" aria-pressed={selected} onClick={onSelect}>
      <span className="volume-option__name">{volume.name}</span>
      <span className="volume-option__capacity">
        {used !== null && total
          ? `${formatBytes(used)} of ${formatBytes(total)}`
          : "Capacity unknown"}
      </span>
      <span className="volume-option__meta">
        {describeVolume(volume)}
        {share !== null && ` · ${formatPercent(share)} used`}
      </span>
    </button>
  );
}

/**
 * A plain description of what a volume is. Every field the platform could not
 * determine is simply absent rather than guessed.
 */
function describeVolume(volume: VolumeInfo): string {
  const parts: string[] = [];
  switch (volume.kind) {
    case "internal":
      parts.push("Internal");
      break;
    case "external":
      parts.push("External");
      break;
    case "network":
      parts.push("Network");
      break;
    case "virtual":
      parts.push("Virtual");
      break;
    default:
      break;
  }
  if (volume.fsType) parts.push(volume.fsType.toUpperCase());
  if (volume.isReadOnly) parts.push("Read only");
  if (!volume.isReadable) parts.push("Not readable by DiskMate");
  parts.push(volume.mountPoint);
  return parts.join(" · ");
}

function folderName(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const cut = trimmed.lastIndexOf("/");
  return cut >= 0 ? trimmed.slice(cut + 1) || "/" : trimmed;
}

/** The capacity bar shown above the explorer once a volume is chosen. */
export function CapacitySummary({
  volume,
  indexedBytes,
}: {
  volume: VolumeInfo;
  indexedBytes: number | null;
}) {
  const total = volume.totalBytes ?? 0;
  const used = volume.usedBytes ?? 0;

  const segments = [
    // What DiskMate actually walked, and the rest of what the volume reports as
    // used. Keeping them distinct is what stops the product from implying it
    // understands more of the disk than it does.
    ...(indexedBytes !== null
      ? [
          {
            id: "indexed",
            label: "Indexed by DiskMate",
            value: Math.min(indexedBytes, used),
            color: "var(--accent)",
          },
          {
            id: "other-used",
            label: "Used, not indexed",
            value: Math.max(used - indexedBytes, 0),
            color: "var(--grey-5)",
          },
        ]
      : [{ id: "used", label: "Used", value: used, color: "var(--grey-5)" }]),
  ];

  return (
    <Panel
      title={volume.name}
      note={`${formatBytes(volume.availableBytes)} available of ${formatBytes(total)}`}
    >
      <Meter segments={segments} total={total} />
    </Panel>
  );
}
