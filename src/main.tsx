import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { getCurrentWindow } from "@tauri-apps/api/window";

import { AppShell } from "./app/AppShell";
import { MenuPanel } from "./features/menubar/MenuPanel";
import { engineAvailable } from "./lib/ipc";

import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/app.css";
import "./styles/banker.css";

const container = document.getElementById("root");
if (!container) throw new Error("Mr Clean could not find its root element.");

/**
 * The same bundle draws both windows: the app, and the panel that opens from
 * the broom in the menu bar. `?panel` shows the panel in a plain browser.
 */
function isPanel(): boolean {
  if (new URLSearchParams(window.location.search).has("panel")) return true;
  if (!engineAvailable()) return false;
  try {
    return getCurrentWindow().label === "panel";
  } catch {
    return false;
  }
}

const panel = isPanel();
document.documentElement.dataset.window = panel ? "panel" : "main";

createRoot(container).render(<StrictMode>{panel ? <MenuPanel /> : <AppShell />}</StrictMode>);
