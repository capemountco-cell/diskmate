import type { ProcessMemory } from "./types";

/**
 * Names people recognise for processes that only have a bundle identifier.
 *
 * WebKit's content processes belong to whichever apps show web pages (Safari,
 * Mail, and many others), so they are named for what they are rather than
 * listed one by one as "com.apple.WebKit.WebContent".
 */
const KNOWN: Record<string, string> = {
  "com.apple.WebKit.WebContent": "Web pages (WebKit)",
  "com.apple.WebKit.GPU": "Web page graphics (WebKit)",
  "com.apple.WebKit.Networking": "Web page networking (WebKit)",
};

export function displayName(name: string): string {
  return KNOWN[name] ?? name;
}

/**
 * Folds rows that read the same once named, adding up their memory.
 *
 * The first row keeps its process id and whether it can be quit; a folded
 * group that came from several processes is never offered a Quit, because
 * one Quit cannot speak for all of them.
 */
export function foldByName(apps: ProcessMemory[]): ProcessMemory[] {
  const byName = new Map<string, ProcessMemory>();
  for (const app of apps) {
    const name = displayName(app.name);
    const seen = byName.get(name);
    if (seen) {
      byName.set(name, {
        ...seen,
        residentBytes: seen.residentBytes + app.residentBytes,
        instanceCount: seen.instanceCount + app.instanceCount,
        canQuit: false,
      });
    } else {
      byName.set(name, { ...app, name });
    }
  }
  return [...byName.values()].sort((a, b) => b.residentBytes - a.residentBytes);
}
