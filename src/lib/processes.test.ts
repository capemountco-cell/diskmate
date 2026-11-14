import { displayName, foldByName } from "./processes";
import type { ProcessMemory } from "./types";

const row = (
  pid: number,
  name: string,
  residentBytes: number,
  canQuit = true,
): ProcessMemory => ({
  pid,
  name,
  residentBytes,
  instanceCount: 1,
  bundlePath: null,
  canQuit,
});

describe("process names", () => {
  it("names WebKit's content processes for what they are", () => {
    expect(displayName("com.apple.WebKit.WebContent")).toBe(
      "Web pages (Safari, Mail and other apps)",
    );
    expect(displayName("Finder")).toBe("Finder");
  });

  it("folds rows with the same name, adds their memory and offers no Quit", () => {
    const folded = foldByName([
      row(1, "Claude", 1_800),
      row(2, "com.apple.WebKit.WebContent", 500, false),
      row(3, "com.apple.WebKit.WebContent", 400, false),
      row(4, "Dropbox", 700),
    ]);
    expect(folded.map((app) => app.name)).toEqual([
      "Claude",
      "Web pages (Safari, Mail and other apps)",
      "Dropbox",
    ]);
    expect(folded[1]).toMatchObject({ residentBytes: 900, instanceCount: 2, canQuit: false });
    expect(folded[0]).toMatchObject({ pid: 1, canQuit: true });
  });
});
