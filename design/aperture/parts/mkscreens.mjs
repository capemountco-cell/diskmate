import { writeFileSync } from "node:fs";

/* Deterministic pseudo-data so the artboards are stable between seeds. */
const wave = (i, a, b, c) => 0.5 + 0.5 * (Math.sin(i / a) * 0.6 + Math.sin(i / b + 1.3) * 0.3 + Math.sin(i / c + 2.7) * 0.1);

/* ------------------------------------------------------------------ bits */

const toolbar = (crumbs, right) => `    <div class="row" style="height:46px;flex:none;gap:14px;padding:0 22px 0 20px">
      <div class="row" style="gap:2px;color:var(--t3)">
        <svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M14 6l-6 6 6 6"></path></svg>
        <svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" style="opacity:.45"><path d="M10 6l6 6-6 6"></path></svg>
      </div>
      <div class="row" style="gap:6px;font-size:13px;color:var(--t2)">${crumbs}</div>
      <div style="margin-left:auto"></div>
${right}
    </div>`;

const search = `      <div class="row" style="gap:6px;width:186px;height:26px;padding:0 9px;border-radius:8px;background:var(--sunken)">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="var(--t3)" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="6.4"></circle><path d="M16 16l4 4"></path></svg>
        <span style="font-size:12.5px;color:var(--t3)">Filter</span>
      </div>`;

const stat = (value, label, size = 20) =>
  `        <div style="display:flex;flex-direction:column;gap:2px">
          <div class="num" style="font-size:${size}px;font-weight:620;letter-spacing:-.025em">${value}</div>
          <div style="font-size:12px;font-weight:500;color:var(--t3)">${label}</div>
        </div>`;

const divider = `        <div style="width:1px;height:38px;background:var(--line)"></div>`;

const hero = (n, unit, caption) =>
  `        <div class="row" style="gap:8px;align-items:baseline">
          <span class="num" style="font-size:46px;font-weight:630;letter-spacing:-.033em;line-height:1">${n}</span>
          <span style="font-size:19px;font-weight:600;color:var(--t2);letter-spacing:-.02em">${unit}</span>
          <span style="font-size:14px;font-weight:500;color:var(--t3);margin-left:2px">${caption}</span>
        </div>`;

/* ============================================================== SCANNING */

const scanBars = Array.from({ length: 90 }, (_, i) => {
  const ramp = Math.min(1, (i + 2) / 16);
  const v = ramp * (0.18 + 0.82 * wave(i, 6.5, 2.3, 15));
  return Math.max(0.08, Math.min(1, v));
})
  .map(
    (v) =>
      `<span style="flex:1;height:${(v * 100).toFixed(1)}%;border-radius:2px;background:var(--accent);opacity:${(0.45 + v * 0.5).toFixed(2)}"></span>`,
  )
  .join("");

const skeleton = (flex, tint, name, size, opacity) =>
  `          <div style="flex:${flex} 1 0;min-height:0;border-radius:12px;padding:12px 13px;display:flex;flex-direction:column;justify-content:space-between;overflow:hidden;background:linear-gradient(158deg,var(--t-${tint}),var(--e-${tint}));opacity:${opacity}">
            <div class="row" style="gap:7px"><span class="dot" style="background:var(--c-${tint})"></span><span style="font-size:12.5px;font-weight:580;letter-spacing:-.012em">${name}</span></div>
            <div class="num" style="font-size:18px;font-weight:620">${size}</div>
          </div>`;

// A cell DiskMate has reached but not finished measuring. It reads as a
// placeholder rather than as a folder of zero bytes.
const pending = (flex) =>
  `          <div style="flex:${flex} 1 0;min-height:0;border-radius:12px;background:var(--sunken);display:flex;flex-direction:column;justify-content:space-between;padding:12px 13px;overflow:hidden">
            <div class="row" style="gap:7px"><span class="dot" style="background:var(--line2)"></span><span style="width:52px;height:7px;border-radius:999px;background:var(--line)"></span></div>
            <span style="width:34px;height:10px;border-radius:999px;background:var(--line)"></span>
          </div>`;

const scanning = `  <!-- ================================================== main -->
  <div style="flex:1;min-width:0;display:flex;flex-direction:column">

${toolbar(
  `<span style="color:var(--t1);font-weight:560">Macintosh HD</span><span style="opacity:.5">›</span><span>Users</span><span style="opacity:.5">›</span><span>mohit</span><span style="opacity:.5">›</span><span>Library</span>`,
  `      <span class="row" style="gap:6px;height:26px;padding:0 11px;border-radius:8px;background:var(--card);box-shadow:var(--shadow-sm);font-size:12.5px;font-weight:560">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="6.5" y="6.5" width="11" height="11" rx="2"></rect></svg>
        Stop
      </span>`,
)}

    <div style="flex:none;padding:6px 26px 18px 30px;display:flex;flex-direction:column;gap:16px">
      <div class="row" style="gap:26px">
        <div class="row" style="gap:14px">
          <svg width="46" height="46" viewBox="0 0 46 46" style="flex:none">
            <circle cx="23" cy="23" r="19" fill="none" stroke="var(--sunken)" stroke-width="5"></circle>
            <circle cx="23" cy="23" r="19" fill="none" stroke="var(--accent)" stroke-width="5" stroke-linecap="round" stroke-dasharray="34 86" transform="rotate(-64 23 23)"></circle>
          </svg>
          <div style="display:flex;flex-direction:column;gap:3px">
${hero("1,284,401", "", "files seen")}
            <div style="font-size:12px;font-weight:500;color:var(--t3)">Total unknown until the walk finishes</div>
          </div>
        </div>
${divider}
${stat("168 GB", "Measured")}
${stat("00:42", "Elapsed")}
${stat("31,200/s", "Rate")}
        <div class="row" style="margin-left:auto;gap:9px">
          <span class="pill" style="background:var(--accent-soft);color:var(--accent-strong);height:24px;padding:0 10px;font-size:11.5px">
            <span style="width:6px;height:6px;border-radius:999px;background:var(--accent)"></span>
            8 threads
          </span>
        </div>
      </div>

      <div class="row" style="gap:2px;height:54px;align-items:flex-end">${scanBars}</div>

      <div class="row" style="gap:9px">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="var(--t3)" stroke-width="1.9" stroke-linejoin="round" style="flex:none"><path d="M3.6 7.4a2 2 0 0 1 2-2h3.2l2 2.2h7.6a2 2 0 0 1 2 2v8.6a2 2 0 0 1-2 2H5.6a2 2 0 0 1-2-2z"></path></svg>
        <span class="mono" style="font-size:11.5px;color:var(--t3);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">/Users/mohit/Library/Developer/CoreSimulator/Devices/A41F…</span>
      </div>
    </div>

    <div style="flex:1;min-height:0;display:flex;gap:18px;padding:0 22px 22px 30px">
      <div class="card" style="flex:1;min-width:0;padding:10px;display:flex;gap:5px">
        <div style="flex:80 1 0;display:flex;flex-direction:column;gap:5px;min-width:0">
${skeleton(42, "system", "System", "38.1 GB", "1")}
${skeleton(31, "applications", "Applications", "26.4 GB", ".82")}
${pending(7)}
        </div>
        <div style="flex:66 1 0;display:flex;flex-direction:column;gap:5px;min-width:0">
${skeleton(30, "developer", "Developer", "24.9 GB", ".9")}
${skeleton(20, "media", "Media", "14.8 GB", ".62")}
${pending(16)}
        </div>
        <div style="flex:54 1 0;display:flex;flex-direction:column;gap:5px;min-width:0">
${skeleton(22, "caches", "Caches", "16.2 GB", ".74")}
${pending(13)}
${pending(9)}
${pending(10)}
        </div>
      </div>

      <aside class="card" style="width:338px;flex:none;display:flex;flex-direction:column;overflow:hidden">
        <div style="padding:16px 18px 12px"><span class="cap">Biggest so far</span></div>
        <div style="flex:1;min-height:0;padding:0 18px 16px;display:flex;flex-direction:column;gap:10px">
${[
  ["System", "system", "38.1 GB", 100],
  ["Applications", "applications", "26.4 GB", 69],
  ["Developer", "developer", "24.9 GB", 65],
  ["Caches", "caches", "16.2 GB", 42],
  ["Media", "media", "14.8 GB", 39],
  ["Documents", "documents", "9.6 GB", 25],
  ["Downloads", "downloads", "7.2 GB", 19],
  ["Photos", "media", "6.1 GB", 16],
  ["Other", "other", "4.4 GB", 12],
]
  .map(
    ([name, tint, size, pct]) => `          <div style="display:flex;flex-direction:column;gap:5px">
            <div class="row" style="gap:8px">
              <span class="dot" style="background:var(--c-${tint})"></span>
              <span style="font-size:12.5px;font-weight:540">${name}</span>
              <span class="num" style="margin-left:auto;font-size:12.5px;font-weight:600;color:var(--t2)">${size}</span>
            </div>
            <div style="height:6px;border-radius:999px;background:var(--sunken)"><div style="width:${pct}%;height:100%;border-radius:999px;background:var(--c-${tint})"></div></div>
          </div>`,
  )
  .join("\n")}
        </div>

        <div style="margin-top:auto;padding:15px 18px;border-top:1px solid var(--line);display:flex;flex-direction:column;gap:10px">
          <span class="cap">Skipped</span>
          <div class="row" style="gap:10px">
            <span class="tile" style="width:30px;height:30px;background:var(--sunken)">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="var(--t2)" stroke-width="1.8" stroke-linecap="round"><rect x="5" y="10.5" width="14" height="9.5" rx="2.4"></rect><path d="M8.4 10.5V8a3.6 3.6 0 0 1 7.2 0v2.5"></path></svg>
            </span>
            <div style="display:flex;flex-direction:column;gap:1px">
              <span class="num" style="font-size:14px;font-weight:620">312 folders</span>
              <span style="font-size:11.5px;color:var(--t3)">Not readable</span>
            </div>
            <span style="margin-left:auto;color:var(--t3)">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M10 6l6 6-6 6"></path></svg>
            </span>
          </div>
        </div>
      </aside>
    </div>
  </div>
`;

/* =============================================================== CLEANUP */

const FINDINGS = [
  { name: "DerivedData", tint: "developer", size: "2.84", note: "Rebuilds on next build", safe: "Reversible", on: true, glyph: "hammer", inside: [100, 74, 52, 38, 27, 18] },
  { name: "Homebrew cache", tint: "caches", size: "3.91", note: "Re-downloads on demand", safe: "Reversible", on: true, glyph: "box", inside: [100, 88, 61, 44, 30, 21] },
  { name: "Duplicate files", tint: "documents", size: "4.31", note: "39 confirmed sets", safe: "Reversible", on: true, glyph: "copy", inside: [100, 30, 24, 19, 12, 9] },
  { name: "Simulators", tint: "applications", size: "9.62", note: "3 unused since March", safe: "Check first", on: false, glyph: "phone", inside: [100, 96, 92, 40, 22, 14] },
  { name: "Device backups", tint: "media", size: "6.40", note: "Last opened 11 months ago", safe: "Check first", on: false, glyph: "clock", inside: [100, 62, 20] },
  { name: "Trash", tint: "other", size: "1.20", note: "Already deleted", safe: "Reversible", on: false, glyph: "bin", inside: [100, 71, 55, 41, 33, 25, 17, 11] },
];

const GLYPH = {
  hammer: `<path d="M12.5 6.5 8 11l-4 4 5 5 4-4 4.5-4.5"></path><path d="M13 3.5 20.5 11l-3 3L10 6.5z"></path>`,
  box: `<path d="M4 8.5 12 4l8 4.5v7L12 20l-8-4.5z"></path><path d="M4 8.5 12 13l8-4.5M12 13v7"></path>`,
  copy: `<rect x="8.4" y="8.4" width="11.2" height="11.2" rx="2.4"></rect><path d="M15.6 5.6H5.6a1.2 1.2 0 0 0-1.2 1.2v10"></path>`,
  phone: `<rect x="7" y="3.5" width="10" height="17" rx="2.6"></rect><path d="M11 17.4h2"></path>`,
  clock: `<circle cx="12" cy="12" r="8.4"></circle><path d="M12 7.6V12l3 1.8"></path>`,
  bin: `<path d="M5 7.5h14"></path><path d="M9.5 7.5V5.2h5v2.3"></path><path d="M7 7.5 8 19h8l1-11.5"></path>`,
};

const card = (f) => {
  const pill =
    f.safe === "Reversible"
      ? `background:var(--accent-soft);color:var(--accent-strong)`
      : `background:rgba(168,118,42,.13);color:var(--caution)`;
  const check = f.on
    ? `<span class="tile" style="width:22px;height:22px;border-radius:7px;background:var(--accent)"><svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 5.5 9.6 18 4 12.6"></path></svg></span>`
    : `<span class="tile" style="width:22px;height:22px;border-radius:7px;background:var(--card);box-shadow:inset 0 0 0 1.5px var(--line2)"></span>`;
  // The bars are the biggest things inside the finding, on the finding's own
  // scale. They fill the card with the shape of the thing rather than a
  // paragraph explaining it.
  const bars = f.inside
    .map(
      (h) =>
        `<span style="flex:1;height:${h}%;border-radius:4px;background:var(--c-${f.tint});opacity:${(0.3 + (h / 100) * 0.55).toFixed(2)}"></span>`,
    )
    .join("");
  return `        <div class="card" style="padding:16px 16px 15px;display:flex;flex-direction:column;gap:12px;${f.on ? "box-shadow:var(--shadow),inset 0 0 0 1.5px var(--accent)" : ""}">
          <div class="row" style="gap:11px">
            <span class="tile" style="width:36px;height:36px;background:linear-gradient(158deg,var(--t-${f.tint}),var(--e-${f.tint}))">
              <svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="var(--c-${f.tint})" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">${GLYPH[f.glyph]}</svg>
            </span>
            <span style="font-size:13.5px;font-weight:590;letter-spacing:-.013em">${f.name}</span>
            <span style="margin-left:auto">${check}</span>
          </div>
          <div class="row" style="gap:9px;align-items:baseline">
            <span class="num" style="font-size:29px;font-weight:640;letter-spacing:-.03em;line-height:1">${f.size}</span>
            <span style="font-size:13px;font-weight:600;color:var(--t2)">GB</span>
            <span style="margin-left:auto;font-size:11px;color:var(--t3)">${f.inside.length} items</span>
          </div>
          <div class="row" style="flex:1;min-height:52px;gap:5px;align-items:flex-end">${bars}</div>
          <div style="display:flex;flex-direction:column;gap:7px">
            <span class="pill" style="${pill};align-self:flex-start">${f.safe}</span>
            <span style="font-size:11.5px;color:var(--t3);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${f.note}</span>
          </div>
        </div>`;
};

const STEP = {
  list: `<path d="M4.5 7h15M4.5 12h15M4.5 17h9"></path>`,
  shield: `<path d="M12 3.6 5.4 6.2v5.4c0 4 2.8 7.3 6.6 8.8 3.8-1.5 6.6-4.8 6.6-8.8V6.2z"></path><path d="M9.2 12.2 11.3 14.4 15 10.4"></path>`,
  trash: `<path d="M5 7.5h14"></path><path d="M9.5 7.5V5.2h5v2.3"></path><path d="M7 7.5 8 19h8l1-11.5"></path>`,
  undo: `<path d="M4.5 9.5h9.8a5.2 5.2 0 1 1 0 10.4H8"></path><path d="M8 5 4 9.5l4 4.5"></path>`,
};

const step = (glyph, label, tone) => `          <div style="display:flex;flex-direction:column;align-items:center;gap:7px;flex:1">
            <span class="tile" style="width:34px;height:34px;border-radius:999px;background:${tone};">
              <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${STEP[glyph]}</svg>
            </span>
            <span style="font-size:10.5px;font-weight:560;color:var(--t2);text-align:center">${label}</span>
          </div>`;

const cleanup = `  <!-- ================================================== main -->
  <div style="flex:1;min-width:0;display:flex;flex-direction:column">

${toolbar(`<span style="color:var(--t1);font-weight:560">Cleanup</span><span style="opacity:.5">›</span><span>Macintosh HD</span>`, search)}

    <div style="flex:none;padding:6px 26px 16px 30px;display:flex;flex-direction:column;gap:14px">
      <div class="row" style="gap:26px">
${hero("11.8", "GB", "DiskMate can account for")}
${divider}
${stat("6", "Findings")}
${stat("3", "Selected")}
${stat("8.95 GB", "Staged")}
        <div class="row" style="margin-left:auto;gap:8px">
          <span class="btn">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M20 12a8 8 0 1 1-2.4-5.7"></path><path d="M20 4.5V9h-4.5"></path></svg>
            Re-check
          </span>
        </div>
      </div>

      <div class="row" style="gap:2px;height:16px;border-radius:999px;overflow:hidden;background:var(--sunken)">
        <span style="flex:9.62;height:100%;background:var(--c-applications)"></span>
        <span style="flex:6.40;height:100%;background:var(--c-media)"></span>
        <span style="flex:4.31;height:100%;background:var(--c-documents)"></span>
        <span style="flex:3.91;height:100%;background:var(--c-caches)"></span>
        <span style="flex:2.84;height:100%;background:var(--c-developer)"></span>
        <span style="flex:1.20;height:100%;background:var(--c-other)"></span>
      </div>
    </div>

    <div style="flex:1;min-height:0;display:flex;gap:18px;padding:0 22px 22px 30px">
      <div style="flex:1;min-width:0;display:grid;grid-template-columns:repeat(3,minmax(0,1fr));grid-auto-rows:1fr;gap:14px">
${FINDINGS.map(card).join("\n")}
      </div>

      <aside class="card" style="width:338px;flex:none;display:flex;flex-direction:column;overflow:hidden">
        <div class="row" style="padding:16px 18px 12px;justify-content:space-between">
          <span class="cap">Staged</span>
          <span class="num" style="font-size:11px;color:var(--t3)">3 items</span>
        </div>

        <div style="flex:1;min-height:0;padding:0 18px 14px;display:flex;flex-direction:column;gap:8px">
${[
  ["DerivedData", "developer", "2.84 GB"],
  ["Homebrew cache", "caches", "3.91 GB"],
  ["Duplicate files", "documents", "4.31 GB"],
]
  .map(
    ([name, tint, size]) => `          <div class="row" style="gap:10px;padding:9px 11px;border-radius:10px;background:var(--sunken)">
            <span class="dot" style="background:var(--c-${tint})"></span>
            <span style="font-size:12.5px;font-weight:540">${name}</span>
            <span class="num" style="margin-left:auto;font-size:12px;font-weight:600;color:var(--t2)">${size}</span>
          </div>`,
  )
  .join("\n")}
        </div>

        <div style="padding:16px 18px;border-top:1px solid var(--line);display:flex;flex-direction:column;gap:14px">
          <span class="cap">What happens next</span>
          <div class="row" style="position:relative;align-items:flex-start">
            <div style="position:absolute;left:34px;right:34px;top:17px;height:1.5px;background:var(--line);z-index:0"></div>
${step("list", "Your list", "var(--accent);color:#fff")}
${step("shield", "Checked", "var(--accent-soft);color:var(--accent-strong)")}
${step("trash", "Trash", "var(--sunken);color:var(--t2)")}
${step("undo", "Put back", "var(--sunken);color:var(--t2)")}
          </div>
        </div>

        <div style="padding:16px 18px;border-top:1px solid var(--line);display:flex;flex-direction:column;gap:12px">
          <span class="cap">Free space after</span>
          <div style="display:flex;flex-direction:column;gap:9px">
            <div style="display:flex;flex-direction:column;gap:5px">
              <div class="row" style="gap:8px"><span style="font-size:11.5px;color:var(--t2)">Now</span><span class="num" style="margin-left:auto;font-size:12px;font-weight:620">25.6 GB</span></div>
              <div style="height:8px;border-radius:999px;background:var(--sunken)"><div style="width:10.4%;height:100%;border-radius:999px;background:var(--c-other)"></div></div>
            </div>
            <div style="display:flex;flex-direction:column;gap:5px">
              <div class="row" style="gap:8px"><span style="font-size:11.5px;color:var(--t2)">Once the Trash is emptied</span><span class="num" style="margin-left:auto;font-size:12px;font-weight:620">34.6 GB</span></div>
              <div style="height:8px;border-radius:999px;background:var(--sunken)"><div style="width:14.1%;height:100%;border-radius:999px;background:var(--accent)"></div></div>
            </div>
          </div>
        </div>

        <div style="padding:15px 16px;border-top:1px solid var(--line);display:flex;flex-direction:column;gap:11px">
          <div class="row" style="align-items:baseline;gap:8px">
            <span class="num" style="font-size:26px;font-weight:640;letter-spacing:-.03em">8.95</span>
            <span style="font-size:13px;font-weight:600;color:var(--t2)">GB</span>
            <span style="margin-left:auto;font-size:11.5px;color:var(--t3)">3 of 6</span>
          </div>
          <span class="btn btn--primary" style="height:36px;border-radius:10px;font-size:13.5px">Move 3 items to Trash</span>
        </div>
      </aside>
    </div>
  </div>
`;

/* ============================================================ DUPLICATES */

const funnelStep = (label, value, width, tone, sub) =>
  `          <div style="display:flex;flex-direction:column;gap:7px">
            <div class="row" style="gap:10px;align-items:baseline">
              <span class="num" style="font-size:22px;font-weight:640;letter-spacing:-.028em;width:120px">${value}</span>
              <span style="font-size:12.5px;font-weight:530;color:var(--t2)">${label}</span>
              <span style="margin-left:auto;font-size:11.5px;color:var(--t3)">${sub}</span>
            </div>
            <div style="height:30px;border-radius:9px;background:var(--sunken);overflow:hidden">
              <div style="width:${width}%;height:100%;border-radius:9px;background:${tone}"></div>
            </div>
          </div>`;

const setRow = (n, name, each, total, tint) => {
  const chips = Array.from({ length: n }, (_, i) =>
    `<span style="width:26px;height:26px;border-radius:7px;background:linear-gradient(158deg,var(--t-${tint}),var(--e-${tint}));box-shadow:inset 0 0 0 1px var(--line);margin-left:${i === 0 ? 0 : -8}px;z-index:${10 - i}"></span>`,
  ).join("");
  return `          <div class="row" style="gap:13px;padding:12px 14px;border-radius:12px;background:var(--sunken)">
            <div class="row" style="flex:none">${chips}</div>
            <div style="min-width:0;display:flex;flex-direction:column;gap:2px">
              <span style="font-size:13px;font-weight:560;letter-spacing:-.012em">${name}</span>
              <span class="mono" style="font-size:10.5px;color:var(--t3)">${n} copies · ${each} each</span>
            </div>
            <div class="row" style="margin-left:auto;gap:12px">
              <div style="text-align:right;display:flex;flex-direction:column;gap:2px">
                <span class="num" style="font-size:13.5px;font-weight:630">${total}</span>
                <span style="font-size:10.5px;color:var(--t3)">reclaimable</span>
              </div>
              <span class="tile btn" style="width:30px;height:28px;padding:0"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M10 6l6 6-6 6"></path></svg></span>
            </div>
          </div>`;
};

const duplicates = `  <!-- ================================================== main -->
  <div style="flex:1;min-width:0;display:flex;flex-direction:column">

${toolbar(`<span style="color:var(--t1);font-weight:560">Duplicates</span><span style="opacity:.5">›</span><span>Macintosh HD</span>`, search)}

    <div style="flex:none;padding:6px 26px 16px 30px;display:flex;flex-direction:column;gap:14px">
      <div class="row" style="gap:26px">
${hero("4.31", "GB", "in 39 identical sets")}
${divider}
${stat("571 MB", "Actually read")}
${stat("0.26%", "Of the scan")}
${stat("00:09", "Took")}
        <div class="row" style="margin-left:auto;gap:8px">
          <span class="btn">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M20 12a8 8 0 1 1-2.4-5.7"></path><path d="M20 4.5V9h-4.5"></path></svg>
            Search again
          </span>
        </div>
      </div>
    </div>

    <div style="flex:1;min-height:0;display:flex;gap:18px;padding:0 22px 22px 30px">

      <div class="card" style="width:440px;flex:none;padding:18px;display:flex;flex-direction:column;gap:16px">
        <span class="cap">How DiskMate got to 39</span>
        <div style="flex:1;min-height:0;display:flex;flex-direction:column;justify-content:space-around;gap:14px">
${funnelStep("files on disk", "2,038,441", 100, "var(--c-other)", "every file indexed")}
${funnelStep("shared a size", "405", 58, "var(--c-applications)", "index only, nothing opened")}
${funnelStep("matched a sample", "154", 32, "var(--c-developer)", "16 KB from each end")}
${funnelStep("identical sets", "39", 14, "var(--accent)", "read in full")}
        </div>

        <div style="margin-top:auto;padding-top:16px;border-top:1px solid var(--line);display:flex;flex-direction:column;gap:9px">
          <div class="row" style="gap:8px">
            <span class="cap">Bytes opened to prove it</span>
            <span class="num" style="margin-left:auto;font-size:12px;font-weight:620;color:var(--t2)">571 MB of 220 GB</span>
          </div>
          <div style="height:8px;border-radius:999px;background:var(--sunken);overflow:hidden">
            <div style="width:2.4%;height:100%;border-radius:999px;background:var(--accent)"></div>
          </div>
        </div>
      </div>

      <div class="card" style="flex:1;min-width:0;display:flex;flex-direction:column;overflow:hidden">
        <div class="row" style="padding:16px 18px 12px;gap:10px">
          <span class="cap">Sets</span>
          <span class="num" style="font-size:11px;color:var(--t3)">39</span>
          <div class="row" style="margin-left:auto;gap:2px;padding:2px;border-radius:8px;background:var(--sunken)">
            <span class="row" style="height:22px;padding:0 9px;border-radius:6px;background:var(--card);box-shadow:var(--shadow-sm);font-size:11.5px;font-weight:580">Largest</span>
            <span class="row" style="height:22px;padding:0 9px;border-radius:6px;font-size:11.5px;color:var(--t2)">Most copies</span>
          </div>
        </div>
        <div style="padding:0 18px 18px;display:flex;flex-direction:column;gap:9px;overflow:hidden">
${setRow(4, "IMG_4821.MOV", "412 MB", "1.21 GB", "media")}
${setRow(3, "node-v22.4.0.pkg", "184 MB", "368 MB", "developer")}
${setRow(6, "Sample Library.wav", "58 MB", "290 MB", "media")}
${setRow(2, "Annual Review 2025.pdf", "128 MB", "128 MB", "documents")}
${setRow(5, "Xcode_16.2.xip", "22 MB", "88 MB", "applications")}
${setRow(3, "backup-2025-11.tar.gz", "41 MB", "82 MB", "other")}
${setRow(2, "Keynote deck (final).key", "37 MB", "37 MB", "documents")}
${setRow(4, "podcast-ep-114.aiff", "9 MB", "27 MB", "media")}
${setRow(2, "Contract signed.pdf", "24 MB", "24 MB", "documents")}
${setRow(3, "sf-pro-display.zip", "11 MB", "22 MB", "other")}

        </div>
        <div class="row" style="margin-top:auto;gap:10px;padding:14px 18px;border-top:1px solid var(--line)">
          <span style="font-size:12px;color:var(--t3)">Keeps the oldest copy in every set</span>
          <span class="btn btn--primary" style="margin-left:auto">Stage 39 sets</span>
        </div>
      </div>
    </div>
  </div>
`;

/* ================================================================ MEMORY */

const memRaw = Array.from({ length: 64 }, (_, i) => {
  const app = 0.3 + 0.16 * wave(i, 9, 4.3, 17);
  const wired = 0.08 + 0.02 * wave(i + 20, 11, 5, 7);
  const comp = 0.04 + 0.03 * wave(i + 40, 7, 3.1, 13);
  const cached = 0.16 + 0.07 * wave(i + 5, 13, 6.2, 9);
  return { app, wired, comp, cached };
});

// Scaled so the tallest column nearly fills the plot: an honest series that
// renders as a flat strip along the floor tells the reader nothing.
const memPeak = Math.max(...memRaw.map((p) => p.app + p.wired + p.comp + p.cached));
const memBars = memRaw
  .map((p) => ({
    app: (p.app / memPeak) * 0.93,
    wired: (p.wired / memPeak) * 0.93,
    comp: (p.comp / memPeak) * 0.93,
    cached: (p.cached / memPeak) * 0.93,
  }))
  .map(
    (p) => `<span style="flex:1;display:flex;flex-direction:column;justify-content:flex-end;gap:1px;height:100%">
          <span style="height:${(p.comp * 100).toFixed(1)}%;background:var(--c-media);border-radius:2px 2px 0 0"></span>
          <span style="height:${(p.wired * 100).toFixed(1)}%;background:var(--c-developer)"></span>
          <span style="height:${(p.app * 100).toFixed(1)}%;background:var(--accent)"></span>
          <span style="height:${(p.cached * 100).toFixed(1)}%;background:var(--c-other);border-radius:0 0 2px 2px"></span>
        </span>`,
  )
  .join("\n        ");

const proc = (name, tint, mem, pct) => `          <div style="display:flex;flex-direction:column;gap:5px">
            <div class="row" style="gap:9px">
              <span class="tile" style="width:22px;height:22px;border-radius:7px;background:linear-gradient(158deg,var(--t-${tint}),var(--e-${tint}));box-shadow:inset 0 0 0 1px var(--line)"><span style="width:8px;height:8px;border-radius:3px;background:var(--c-${tint})"></span></span>
              <span style="font-size:12.5px;font-weight:540">${name}</span>
              <span class="num" style="margin-left:auto;font-size:12.5px;font-weight:600;color:var(--t2)">${mem}</span>
            </div>
            <div style="height:6px;border-radius:999px;background:var(--sunken)"><div style="width:${pct}%;height:100%;border-radius:999px;background:var(--c-${tint})"></div></div>
          </div>`;

const memory = `  <!-- ================================================== main -->
  <div style="flex:1;min-width:0;display:flex;flex-direction:column">

${toolbar(`<span style="color:var(--t1);font-weight:560">Memory</span><span style="opacity:.5">›</span><span>Live</span>`, `      <div class="row" style="gap:2px;padding:2px;border-radius:9px;background:var(--sunken)">
        <span class="row" style="height:24px;padding:0 10px;border-radius:7px;background:var(--card);box-shadow:var(--shadow-sm);font-size:12.5px;font-weight:590">60s</span>
        <span class="row" style="height:24px;padding:0 10px;border-radius:7px;font-size:12.5px;color:var(--t2)">5m</span>
        <span class="row" style="height:24px;padding:0 10px;border-radius:7px;font-size:12.5px;color:var(--t2)">1h</span>
      </div>`)}

    <div style="flex:none;padding:6px 26px 16px 30px;display:flex;flex-direction:column;gap:14px">
      <div class="row" style="gap:26px">
${hero("61", "%", "of 32 GB in use")}
${divider}
${stat("12.4 GB", "App")}
${stat("3.2 GB", "Wired")}
${stat("1.8 GB", "Compressed")}
${stat("0 B", "Swap")}
        <div class="row" style="margin-left:auto;gap:8px">
          <span class="pill" style="background:var(--accent-soft);color:var(--accent-strong);height:24px;padding:0 10px;font-size:11.5px">
            <span style="width:6px;height:6px;border-radius:999px;background:var(--accent)"></span>
            Comfortable
          </span>
        </div>
      </div>
    </div>

    <div style="flex:1;min-height:0;display:flex;gap:18px;padding:0 22px 22px 30px">

      <div class="card" style="flex:1;min-width:0;padding:18px;display:flex;flex-direction:column;gap:14px">
        <div class="row" style="gap:18px">
          <span class="cap">Last 60 seconds</span>
          <div class="row" style="margin-left:auto;gap:16px">
            <span class="row" style="gap:6px"><span class="dot" style="background:var(--c-media)"></span><span style="font-size:11.5px;color:var(--t2)">Compressed</span></span>
            <span class="row" style="gap:6px"><span class="dot" style="background:var(--c-developer)"></span><span style="font-size:11.5px;color:var(--t2)">Wired</span></span>
            <span class="row" style="gap:6px"><span class="dot" style="background:var(--accent)"></span><span style="font-size:11.5px;color:var(--t2)">App</span></span>
            <span class="row" style="gap:6px"><span class="dot" style="background:var(--c-other)"></span><span style="font-size:11.5px;color:var(--t2)">Cached</span></span>
          </div>
        </div>

        <div class="row" style="flex:1;min-height:0;gap:2px;align-items:stretch">
        ${memBars}
        </div>

        <div class="row" style="justify-content:space-between;padding-top:2px">
          <span class="num" style="font-size:11px;color:var(--t3)">60s ago</span>
          <span class="num" style="font-size:11px;color:var(--t3)">now</span>
        </div>

        <div class="row" style="gap:14px;padding-top:14px;border-top:1px solid var(--line)">
          <div style="flex:1;display:flex;flex-direction:column;gap:7px">
            <div class="row" style="gap:8px"><span class="cap">Swap used</span><span class="num" style="margin-left:auto;font-size:12px;font-weight:620">0 B</span></div>
            <div style="height:8px;border-radius:999px;background:var(--sunken)"></div>
          </div>
          <div style="flex:1;display:flex;flex-direction:column;gap:7px">
            <div class="row" style="gap:8px"><span class="cap">Compression ratio</span><span class="num" style="margin-left:auto;font-size:12px;font-weight:620">2.4×</span></div>
            <div style="height:8px;border-radius:999px;background:var(--sunken)"><div style="width:41%;height:100%;border-radius:999px;background:var(--c-media)"></div></div>
          </div>
        </div>
      </div>

      <aside class="card" style="width:338px;flex:none;display:flex;flex-direction:column;overflow:hidden">
        <div class="row" style="padding:16px 18px 12px;justify-content:space-between">
          <span class="cap">Holding the most</span>
          <span class="num" style="font-size:11px;color:var(--t3)">of 412</span>
        </div>
        <div style="flex:1;min-height:0;padding:0 18px 16px;display:flex;flex-direction:column;gap:11px">
${proc("Google Chrome", "applications", "3.84 GB", 100)}
${proc("Xcode", "developer", "2.91 GB", 76)}
${proc("Final Cut Pro", "media", "1.62 GB", 42)}
${proc("Simulator", "applications", "1.08 GB", 28)}
${proc("Slack", "applications", "842 MB", 21)}
${proc("WindowServer", "system", "611 MB", 16)}
${proc("Safari", "applications", "588 MB", 15)}
${proc("Photos", "media", "441 MB", 11)}
${proc("Music", "media", "305 MB", 8)}
${proc("Figma", "applications", "288 MB", 7)}
${proc("Docker", "developer", "241 MB", 6)}
        </div>

        <div style="margin-top:auto;padding:16px 18px;border-top:1px solid var(--line);display:flex;flex-direction:column;gap:11px">
          <div class="row" style="gap:11px">
            <span class="tile" style="width:32px;height:32px;background:var(--sunken)">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--t2)" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="8.4"></circle><path d="M12 11v5.4"></path><circle cx="12" cy="8" r="1" fill="var(--t2)" stroke="none"></circle></svg>
            </span>
            <span style="font-size:11.5px;line-height:1.45;color:var(--t2)">DiskMate reads memory. It never frees, purges or quits anything.</span>
          </div>
        </div>
      </aside>
    </div>
  </div>
`;

writeFileSync("Scanning.main.html", scanning);
writeFileSync("Cleanup.main.html", cleanup);
writeFileSync("Duplicates.main.html", duplicates);
writeFileSync("Memory.main.html", memory);
console.log("wrote 4 main columns");
