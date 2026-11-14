import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ScanProgress } from "../../lib/types";

import { ScanStatusBar } from "./ScanStatusBar";

const progress = (bytesIndexed: number): ScanProgress => ({
  scanId: 1,
  status: "running",
  entriesScanned: 0,
  filesScanned: 412_880,
  directoriesScanned: 68_214,
  symlinksScanned: 0,
  bytesIndexed,
  errors: 0,
  elapsedMs: 14_000,
  entriesPerSecond: 0,
  currentPath: null,
});

describe("ScanStatusBar", () => {
  it("shows bytes read against the space the volume reports as used", () => {
    render(
      <ScanStatusBar
        progress={progress(96_300_000_000)}
        onCancel={() => {}}
        usedBytes={280_000_000_000}
        targetName="Macintosh HD"
      />,
    );
    const bar = screen.getByRole("progressbar");
    expect(bar.getAttribute("aria-valuenow")).toBe("34");
    expect(screen.getByText("34%")).toBeTruthy();
  });

  it("never claims to be finished while the scan is still running", () => {
    render(
      <ScanStatusBar
        progress={progress(300_000_000_000)}
        onCancel={() => {}}
        usedBytes={280_000_000_000}
        targetName="Macintosh HD"
      />,
    );
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("99");
  });

  it("shows no bar at all when there is no total to measure against", () => {
    render(
      <ScanStatusBar
        progress={progress(1_000_000)}
        onCancel={() => {}}
        usedBytes={null}
        targetName="Projects"
      />,
    );
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.getByText(/1.0 MB read|1 MB read/)).toBeTruthy();
  });
});
