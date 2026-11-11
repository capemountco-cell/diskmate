import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { AppShell } from "./app/AppShell";

import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/app.css";
import "./styles/vivid.css";

const container = document.getElementById("root");
if (!container) throw new Error("DiskMate could not find its root element.");

createRoot(container).render(
  <StrictMode>
    <AppShell />
  </StrictMode>,
);
