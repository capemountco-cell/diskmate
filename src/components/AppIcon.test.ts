import { appBundleOf } from "./AppIcon";

describe("appBundleOf", () => {
  it("finds the app a path belongs to", () => {
    expect(appBundleOf("/Applications/Docker.app")).toBe("/Applications/Docker.app");
    expect(appBundleOf("/Applications/Docker.app/Contents/MacOS/Docker")).toBe(
      "/Applications/Docker.app",
    );
  });

  it("ignores paths that only look like apps", () => {
    expect(appBundleOf("/usr/libexec/applepushd")).toBeNull();
    expect(appBundleOf("/Library/Thing.application/x")).toBeNull();
    expect(appBundleOf(null)).toBeNull();
  });
});
