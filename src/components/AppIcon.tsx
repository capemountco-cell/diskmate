import { useEffect, useState } from "react";

import { engine, engineAvailable } from "../lib/ipc";

/** Icons asked for once per path for as long as the window is open. */
const cache = new Map<string, Promise<string | null>>();

/** The pixel size asked for: enough for a 40-point tile on a Retina screen. */
const PIXELS = 96;

/**
 * The `.app` a path belongs to: itself, or the bundle it sits inside
 * (`/Applications/Docker.app/Contents/MacOS/Docker` → `/Applications/Docker.app`).
 */
export function appBundleOf(path: string | null | undefined): string | null {
  if (!path) return null;
  const at = path.indexOf(".app");
  if (at < 0) return null;
  const end = at + ".app".length;
  if (end < path.length && path[end] !== "/") return null;
  return path.slice(0, end);
}

function load(path: string): Promise<string | null> {
  let pending = cache.get(path);
  if (!pending) {
    pending = engine.appIcon(path, PIXELS).catch(() => null);
    cache.set(path, pending);
  }
  return pending;
}

/**
 * An app's real icon, as Finder shows it, read from macOS.
 *
 * While it loads, and for anything with no icon (background processes, apps
 * macOS cannot draw), it shows a neutral tile with the first letter of the
 * name instead.
 */
export function AppIcon({
  path,
  name,
  size = 38,
}: {
  path: string | null | undefined;
  name: string;
  size?: number;
}) {
  const bundle = appBundleOf(path);
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    setSrc(null);
    if (!bundle || !engineAvailable()) return;
    let live = true;
    void load(bundle).then((url) => live && setSrc(url));
    return () => {
      live = false;
    };
  }, [bundle]);

  if (src) {
    return (
      <img
        className="app-icon"
        src={src}
        alt=""
        width={size}
        height={size}
        draggable={false}
        aria-hidden="true"
      />
    );
  }
  return (
    <span
      className="app-tile"
      style={{ width: size, height: size, flexBasis: size, fontSize: Math.round(size * 0.4) }}
      aria-hidden="true"
    >
      {name.trim().charAt(0).toUpperCase() || "?"}
    </span>
  );
}
