import type { ProcessMemory } from "./types";

/**
 * Names people recognise for processes that only have a bundle identifier.
 *
 * WebKit's processes draw the web pages of every app that shows one: Safari
 * mostly, but also Mail, Messages, the App Store and others. macOS only says
 * which app each belongs to through a private interface Mr Clean does not
 * use, so they are named for what they are, with the usual owners, rather
 * than credited to Safari on a guess.
 */
const KNOWN: Record<string, string> = {
  "com.apple.WebKit.WebContent": "Web pages (Safari, Mail and other apps)",
  "com.apple.WebKit.GPU": "Web page graphics (Safari and other apps)",
  "com.apple.WebKit.Networking": "Web page networking (Safari and other apps)",
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
