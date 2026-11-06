import { Button, EmptyState, Note } from "../../components/primitives";
import { formatBytes, formatPercent } from "../../lib/format";
import type { VolumeInfo } from "../../lib/types";

/**
 * Which disk, in one row.
 *
 * This used to be a panel of tall rows, each with a name, a capacity and a
 * third line spelling out "Internal · APFS · Read only · /Volumes/…". That line
 * is worth having when you need it and noise the rest of the time, so it is the
 * chip's tooltip now and the capacity is a bar you can read without reading.
 */
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
  const folderSelected =
    selectedRoot !== null && !volumes.some((v) => v.mountPoint === selectedRoot);

  if (error) return <Note tone="caution">{error}</Note>;

  if (!loading && volumes.length === 0 && !folderSelected) {
    return (
      <EmptyState title="No volumes found">
        DiskMate could not read the list of mounted volumes on this system.
      </EmptyState>
    );
  }

  return (
    <div className="volume-bar">
      {loading && volumes.length === 0 && <span className="muted">Looking for volumes…</span>}

      {volumes.map((volume) => (
        <VolumeChip
          key={volume.mountPoint}
          volume={volume}
          selected={volume.mountPoint === selectedRoot}
          onSelect={() => onSelect(volume.mountPoint)}
        />
      ))}

      {folderSelected && (
        <span className="volume-chip" aria-current="true" title={selectedRoot!}>
          <span className="volume-chip__name">{folderName(selectedRoot!)}</span>
          <span className="volume-chip__size">Folder</span>
        </span>
      )}

      <span className="volume-bar__actions">
        <Button variant="quiet" onClick={onChooseFolder}>
          Choose a folder…
        </Button>
        <Button variant="quiet" onClick={onRefresh} disabled={loading}>
          Refresh
        </Button>
      </span>
    </div>
  );
}

function VolumeChip({
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
    <button
      type="button"
      className={selected ? "volume-chip volume-chip--selected" : "volume-chip"}
      aria-pressed={selected}
      onClick={onSelect}
      title={describeVolume(volume)}
    >
      <span className="volume-chip__name">{volume.name}</span>
      {share !== null ? (
        <>
          <span className="volume-chip__meter" aria-hidden="true">
            <span style={{ width: `${Math.min(100, share * 100)}%` }} />
          </span>
          <span className="volume-chip__size num">
            {formatBytes(used!)} of {formatBytes(total!)}
          </span>
        </>
      ) : (
        <span className="volume-chip__size">Capacity unknown</span>
      )}
      {!volume.isReadable && <span className="volume-chip__flag">Not readable</span>}
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
  const used = volume.usedBytes;
  const total = volume.totalBytes;
  if (used !== null && total) parts.push(`${formatPercent(used / total)} used`);
  return parts.join(" · ");
}

function folderName(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const cut = trimmed.lastIndexOf("/");
  return cut >= 0 ? trimmed.slice(cut + 1) || "/" : trimmed;
}
