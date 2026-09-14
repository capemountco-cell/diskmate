import { CleanupIcon, MemoryIcon, StartupIcon, StorageIcon } from "../components/icons";
import type { AppInfo } from "../lib/types";

export type SectionId = "storage" | "cleanup" | "startup" | "memory";

export const SECTIONS: Array<{
  id: SectionId;
  label: string;
  title: string;
  icon: (props: { size?: number; className?: string }) => React.ReactElement;
  status?: string;
}> = [
  {
    id: "storage",
    label: "Storage",
    title: "Storage",
    icon: StorageIcon,
  },
  {
    id: "cleanup",
    label: "Cleanup",
    title: "Cleanup",
    icon: CleanupIcon,
  },
  {
    id: "startup",
    label: "Startup",
    title: "Startup",
    icon: StartupIcon,
  },
  {
    id: "memory",
    label: "Memory",
    title: "Memory",
    icon: MemoryIcon,
  },
];

export function Sidebar({
  active,
  onSelect,
  info,
}: {
  active: SectionId;
  onSelect: (id: SectionId) => void;
  info: AppInfo | null;
}) {
  return (
    <aside className="sidebar">
      <div className="sidebar__brand">
        <span className="sidebar__mark" aria-hidden="true">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="12" cy="12" r="8.2" />
            <circle cx="12" cy="12" r="2.2" />
          </svg>
        </span>
        <div className="sidebar__title">DiskMate</div>
      </div>

      <nav className="sidebar__nav" aria-label="Sections">
        {SECTIONS.map((section) => {
          const Icon = section.icon;
          return (
            <button
              key={section.id}
              type="button"
              className="nav-item"
              aria-current={section.id === active ? "page" : undefined}
              onClick={() => onSelect(section.id)}
            >
              <span className="nav-item__icon">
                <Icon size={15} />
              </span>
              {section.label}
              {section.status && <span className="nav-item__badge">{section.status}</span>}
            </button>
          );
        })}
      </nav>

      <div
        className="sidebar__footer"
        title={
          info
            ? `${info.storageMode === "portable" ? "Running portably" : "Version " + info.version} · ${info.databasePath}`
            : undefined
        }
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
          <rect x="5" y="10.5" width="14" height="9.5" rx="2.4" />
          <path d="M8.4 10.5V8a3.6 3.6 0 0 1 7.2 0v2.5" />
        </svg>
        Nothing leaves this Mac
      </div>
    </aside>
  );
}
