import {
  CleanupIcon,
  MemoryIcon,
  StartupIcon,
  StorageIcon,
} from "../components/icons";
import type { AppInfo } from "../lib/types";

export type SectionId = "storage" | "cleanup" | "startup" | "memory";

export const SECTIONS: Array<{
  id: SectionId;
  label: string;
  title: string;
  subtitle: string;
  icon: (props: { size?: number; className?: string }) => React.ReactElement;
  status?: string;
}> = [
  {
    id: "storage",
    label: "Storage",
    title: "Storage",
    subtitle: "Where your storage went.",
    icon: StorageIcon,
  },
  {
    id: "cleanup",
    label: "Cleanup",
    title: "Cleanup",
    subtitle: "What you can safely remove.",
    icon: CleanupIcon,
    status: "Soon",
  },
  {
    id: "startup",
    label: "Startup",
    title: "Startup",
    subtitle: "What runs when your Mac starts.",
    icon: StartupIcon,
  },
  {
    id: "memory",
    label: "Memory",
    title: "Memory",
    subtitle: "How your Mac is using its memory.",
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
        <div className="sidebar__title">DiskMate</div>
        <div className="sidebar__tagline">Storage, understood.</div>
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

      <div className="sidebar__footer">
        <div>Everything stays on this Mac.</div>
        {info && (
          <div title={info.databasePath}>
            {info.storageMode === "portable" ? "Running portably" : "Version " + info.version}
          </div>
        )}
      </div>
    </aside>
  );
}
