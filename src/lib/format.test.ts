import { describe, expect, it } from "vitest";

import { formatBytes, formatCount, formatPercent, truncatePath } from "./format";

describe("formatBytes", () => {
  it("uses decimal units so Mr Clean agrees with Finder", () => {
    expect(formatBytes(1000)).toBe("1.0 KB");
    expect(formatBytes(1_500_000)).toBe("1.5 MB");
    expect(formatBytes(8_300_000_000)).toBe("8.3 GB");
  });

  it("drops the decimal once the number is large enough not to need it", () => {
    expect(formatBytes(412_000_000_000)).toBe("412 GB");
  });

  it("says zero rather than showing an empty cell", () => {
    expect(formatBytes(0)).toBe("Zero bytes");
  });

  it("distinguishes unknown from zero", () => {
    expect(formatBytes(null)).toBe("—");
    expect(formatBytes(undefined)).toBe("—");
  });

  it("keeps small values in bytes", () => {
    expect(formatBytes(512)).toBe("512 bytes");
  });
});

describe("formatPercent", () => {
  it("never rounds a real share down to zero", () => {
    expect(formatPercent(0.0001)).toBe("<0.1%");
  });

  it("drops decimals above ten percent", () => {
    expect(formatPercent(0.4237)).toBe("42%");
    expect(formatPercent(0.0512)).toBe("5.1%");
  });
});

describe("formatCount", () => {
  it("groups thousands", () => {
    expect(formatCount(841293)).toBe(new Intl.NumberFormat().format(841293));
  });
});

describe("truncatePath", () => {
  it("leaves short paths alone", () => {
    expect(truncatePath("/Users/ada/Downloads")).toBe("/Users/ada/Downloads");
  });

  it("keeps both ends of a long path", () => {
    const long =
      "/Users/ada/Library/Developer/Xcode/DerivedData/SomeProject/Build/Products/Debug";
    const result = truncatePath(long, 40);
    expect(result.startsWith("/Users")).toBe(true);
    expect(result.endsWith("Products/Debug")).toBe(true);
    expect(result).toContain("…");
  });
});
