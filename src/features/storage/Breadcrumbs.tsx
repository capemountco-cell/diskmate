import { ChevronRightIcon } from "../../components/icons";
import type { EntryRow } from "../../lib/types";

/**
 * The path from the scan root to the open folder.
 *
 * Long paths collapse in the middle rather than scrolling, keeping the root and
 * the current folder, which are the two crumbs anyone actually needs.
 */
export function Breadcrumbs({
  trail,
  onNavigate,
}: {
  trail: EntryRow[];
  onNavigate: (entry: EntryRow) => void;
}) {
  const visible = collapse(trail);

  return (
    <nav className="breadcrumbs" aria-label="Folder path">
      {visible.map((entry, index) => {
        if (entry === "gap") {
          return (
            <span key="gap" className="breadcrumbs__separator" aria-hidden="true">
              <ChevronRightIcon size={12} /> …
            </span>
          );
        }
        const isLast = index === visible.length - 1;
        return (
          <span key={entry.id} style={{ display: "contents" }}>
            {index > 0 && (
              <span className="breadcrumbs__separator" aria-hidden="true">
                <ChevronRightIcon size={12} />
              </span>
            )}
            <button
              type="button"
              className="breadcrumbs__crumb"
              aria-current={isLast ? "page" : undefined}
              onClick={() => !isLast && onNavigate(entry)}
              title={entry.path}
            >
              {entry.name}
            </button>
          </span>
        );
      })}
    </nav>
  );
}

const MAX_CRUMBS = 5;

function collapse(trail: EntryRow[]): Array<EntryRow | "gap"> {
  if (trail.length <= MAX_CRUMBS) return trail;
  const first = trail[0];
  if (!first) return trail;
  return [first, "gap", ...trail.slice(-(MAX_CRUMBS - 2))];
}
