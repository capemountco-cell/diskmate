import { Button } from "../../components/primitives";
import { formatBytes, formatCount, formatDuration, truncatePath } from "../../lib/format";
import type { ScanProgress } from "../../lib/types";

/**
 * Live scan feedback.
 *
 * The track is indeterminate on purpose. The scanner cannot know how many
 * entries a volume holds until it has walked it, so a percentage would be
 * invented, and an invented percentage that jumps around is worse than none.
 * The numbers underneath are real and update several times a second.
 */
export function ScanStatusBar({
  progress,
  onCancel,
}: {
  progress: ScanProgress | null;
  onCancel: () => void;
}) {
  return (
    <div className="scan-bar">
      <div className="row row--between">
        <strong>Scanning…</strong>
        <Button variant="quiet" onClick={onCancel}>
          Cancel
        </Button>
      </div>

      <div className="scan-bar__track">
        <div className="scan-bar__indicator" />
      </div>

      <div className="scan-bar__stats">
        <span>{formatCount(progress?.filesScanned ?? 0)} files</span>
        <span>{formatCount(progress?.directoriesScanned ?? 0)} folders</span>
        <span>{formatBytes(progress?.bytesIndexed ?? 0)} indexed</span>
        <span>{formatDuration(progress?.elapsedMs ?? 0)}</span>
        {progress && progress.entriesPerSecond > 0 && (
          <span className="faint">{formatCount(progress.entriesPerSecond)} entries/second</span>
        )}
        {progress && progress.errors > 0 && (
          <span className="faint">{formatCount(progress.errors)} skipped</span>
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
