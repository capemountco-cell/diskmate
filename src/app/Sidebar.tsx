import {
  ApplicationsIcon,
  CleanupIcon,
  DuplicatesIcon,
  MacIcon,
  MemoryIcon,
  StartupIcon,
  StorageIcon,
} from "../components/icons";
import type { AppInfo } from "../lib/types";

export type SectionId =
  "overview" | "storage" | "cleanup" | "duplicates" | "applications" | "startup" | "memory";

export const SECTIONS: Array<{
  id: SectionId;
  label: string;
  icon: (props: { size?: number; className?: string }) => React.ReactElement;
}> = [
  { id: "overview", label: "This Mac", icon: MacIcon },
  { id: "storage", label: "Storage", icon: StorageIcon },
  { id: "cleanup", label: "Cleanup", icon: CleanupIcon },
  { id: "duplicates", label: "Duplicates", icon: DuplicatesIcon },
  { id: "applications", label: "Applications", icon: ApplicationsIcon },
  { id: "startup", label: "Startup", icon: StartupIcon },
  { id: "memory", label: "Memory", icon: MemoryIcon },
];

/**
 * Navigation.
 *
 * Two shapes of the same list. On a screen that is introducing a section the
 * sidebar carries labels, because that is when someone is still finding their
 * way; once a screen is showing results it folds to an icon rail and gives the
 * room to the results. The labels never disappear entirely: every icon keeps
 * its name for assistive technology and as a tooltip.
 */
export function Sidebar({
  active,
  onSelect,
  info,
  expanded,
  marked,
}: {
  active: SectionId;
  onSelect: (id: SectionId) => void;
  info: AppInfo | null;
  expanded: boolean;
  /** Sections that hold results worth returning to. */
  marked: ReadonlySet<SectionId>;
}) {
  return (
    <aside className={expanded ? "sidebar sidebar--expanded" : "sidebar"}>
      <nav className="sidebar__nav" aria-label="Sections">
        {SECTIONS.map((section) => {
          const Icon = section.icon;
          return (
            <button
              key={section.id}
              type="button"
              className="nav-item"
              aria-current={section.id === active ? "page" : undefined}
              aria-label={expanded ? undefined : section.label}
              title={expanded ? undefined : section.label}
              data-marked={marked.has(section.id) || undefined}
              onClick={() => onSelect(section.id)}
            >
              <span className="nav-item__icon">
                <Icon size={20} />
              </span>
              {expanded && <span className="nav-item__label">{section.label}</span>}
            </button>
          );
        })}
      </nav>

      <div className="sidebar__footer">
        {expanded ? (
          <>
            <div>Everything stays on this Mac.</div>
            {info && (
              <div title={info.databasePath}>
                {info.storageMode === "portable"
                  ? "Running portably"
                  : "Version " + info.version}
              </div>
            )}
          </>
        ) : (
          info && (
            <div title={`DiskMate ${info.version}. Everything stays on this Mac.`}>
              {info.version}
            </div>
          )
        )}
      </div>
    </aside>
  );
}
