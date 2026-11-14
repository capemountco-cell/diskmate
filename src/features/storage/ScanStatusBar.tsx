import { Button } from "../../components/primitives";
import {
  formatBytes,
  formatCount,
  formatDuration,
  formatPercent,
  truncatePath,
} from "../../lib/format";
import type { ScanProgress } from "../../lib/types";

/**
 * Live scan feedback.
 *
 * The scanner cannot know how many files a volume holds until it has walked
 * it, so a file-count percentage would be invented. What can be measured is
 * how many bytes have been read against how many the volume itself reports as
 * used, and that is what the bar shows, labelled as exactly that.
 *
 * It is not a time estimate, and it can finish short of 100%: space the volume
 * counts as used can include snapshots, other volumes in the same container
 * and folders macOS will not let Mr Clean read. So the bar stops at 99% while
 * the scan runs, and the label always gives the two real numbers.
 *
 * A folder scan has no such total, so it shows the live counts and no bar at
 * all, rather than an animation that looks like progress and is not.
 */
export function ScanStatusBar({
  progress,
  onCancel,
  usedBytes,
  targetName,
}: {
  progress: ScanProgress | null;
  onCancel: () => void;
  /** Bytes the volume reports as used, or null when scanning a folder. */
  usedBytes: number | null;
  targetName: string;
}) {
  const read = progress?.bytesIndexed ?? 0;
  const share = usedBytes && usedBytes > 0 ? Math.min(read / usedBytes, 0.99) : null;

  return (
    <div className="scan-bar">
      <div className="row row--between">
        <strong>Reading {targetName}…</strong>
        <Button variant="quiet" onClick={onCancel}>
          Stop
        </Button>
      </div>

      {share !== null && usedBytes !== null && (
        <>
          <div
            className="scan-bar__track"
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(share * 100)}
            aria-label={`${formatBytes(read)} read of the ${formatBytes(usedBytes)} ${targetName} reports as used`}
          >
            <div className="scan-bar__fill" style={{ width: `${share * 100}%` }} />
          </div>
          <div className="scan-bar__measure">
            <span>
              <b>{formatBytes(read)}</b> read of the <b>{formatBytes(usedBytes)}</b>{" "}
              {targetName} reports as used
            </span>
            <b className="num">{formatPercent(share, 0)}</b>
          </div>
        </>
      )}

      <div className="scan-bar__stats">
        <span>{formatCount(progress?.filesScanned ?? 0)} files</span>
        <span>{formatCount(progress?.directoriesScanned ?? 0)} folders</span>
        {share === null && <span>{formatBytes(read)} read</span>}
        <span>{formatDuration(progress?.elapsedMs ?? 0)}</span>
        {progress && progress.errors > 0 && (
          <span className="faint">{formatCount(progress.errors)} could not be read</span>
        )}
      </div>

      {progress?.currentPath && (
        <div className="scan-bar__path" title={progress.currentPath}>
          {truncatePath(progress.currentPath, 90)}
        </div>
      )}
    </div>
  );
}
