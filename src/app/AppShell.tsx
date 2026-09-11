import { useEffect, useState } from "react";

import { Note } from "../components/primitives";
import { CleanupPage } from "../features/cleanup/CleanupPage";
import { MemoryPage } from "../features/memory/MemoryPage";
import { StartupPage } from "../features/startup/StartupPage";
import { StoragePage } from "../features/storage/StoragePage";
import { engine, engineAvailable } from "../lib/ipc";
import type { AppInfo } from "../lib/types";

import { SECTIONS, Sidebar, type SectionId } from "./Sidebar";

export function AppShell() {
  const [section, setSection] = useState<SectionId>("storage");
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    if (!engineAvailable()) return;
    engine
      .appInfo()
      .then(setInfo)
      .catch(() => setInfo(null));
  }, []);

  const current = SECTIONS.find((entry) => entry.id === section) ?? SECTIONS[0]!;

  return (
    <div className="shell">
      <Sidebar active={section} onSelect={setSection} info={info} />

      <main className="main">
        <header className="main__header">
          <h1 className="page-title">{current.title}</h1>
          <p className="page-subtitle">{current.subtitle}</p>
        </header>

        <div className="main__body">
          {info?.portableFallbackReason && (
            <div style={{ marginBottom: "var(--space-4)" }}>
              <Note tone="caution" title="Portable mode is not active.">
                {info.portableFallbackReason} DiskMate is storing its index on this Mac instead,
                at {info.dataDir}.
              </Note>
            </div>
          )}

          {section === "storage" && <StoragePage />}
          {section === "cleanup" && <CleanupPage />}
          {section === "startup" && <StartupPage />}
          {section === "memory" && <MemoryPage />}
        </div>
      </main>
    </div>
  );
}
