// Emits the shared sidebar for one screen. The nav selection is the only thing
// that differs between artboards, so it is generated rather than copied six
// times and allowed to drift.
const active = process.argv[2] ?? "storage";

const NAV = [
  {
    id: "storage",
    label: "Storage",
    badge: null,
    icon: `<circle cx="12" cy="12" r="8.2"></circle><circle cx="12" cy="12" r="2.4"></circle>`,
  },
  {
    id: "cleanup",
    label: "Cleanup",
    badge: "11.8 GB",
    icon: `<path d="M5 7.5h14"></path><path d="M9.5 7.5V5.2h5v2.3"></path><path d="M7 7.5 8 19h8l1-11.5"></path>`,
  },
  {
    id: "startup",
    label: "Startup",
    badge: "24",
    icon: `<path d="M13.2 3.2 6.5 13.4h4.7l-1 7.4 7-10.4h-4.7z"></path>`,
  },
  {
    id: "memory",
    label: "Memory",
    badge: "61%",
    icon: `<rect x="7.2" y="7.2" width="9.6" height="9.6" rx="2.2"></rect><path d="M10 4.2v3M14 4.2v3M10 16.8v3M14 16.8v3M4.2 10h3M4.2 14h3M16.8 10h3M16.8 14h3"></path>`,
  },
];

const item = (n) => {
  const on = n.id === active;
  const row = on
    ? `gap:9px;height:32px;padding:0 8px;border-radius:8px;background:var(--accent-soft);color:var(--accent-strong)`
    : `gap:9px;height:32px;padding:0 8px;border-radius:8px;color:var(--t2)`;
  const label = on
    ? `font-size:13.5px;font-weight:590;letter-spacing:-.01em`
    : `font-size:13.5px;font-weight:500;letter-spacing:-.01em;color:var(--t1)`;
  const badge =
    n.badge === null
      ? ``
      : `\n        <span class="num" style="margin-left:auto;font-size:12px;font-weight:600;color:${
          on ? "var(--accent-strong)" : "var(--t3)"
        }">${n.badge}</span>`;
  return `      <div class="row" style="${row}">
        <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${n.icon}</svg>
        <span style="${label}">${n.label}</span>${badge}
      </div>`;
};

process.stdout.write(`  <!-- ================================================== sidebar -->
  <aside style="width:248px;flex:none;display:flex;flex-direction:column;gap:20px;padding:13px 12px 12px;background:var(--sidebar);border-right:1px solid var(--line)">

    <div class="row" style="gap:8px;padding:1px 6px 0">
      <span style="width:12px;height:12px;border-radius:999px;background:#ff5f57"></span>
      <span style="width:12px;height:12px;border-radius:999px;background:#febc2e"></span>
      <span style="width:12px;height:12px;border-radius:999px;background:#28c840"></span>
    </div>

    <div class="row" style="gap:9px;padding:0 6px">
      <span class="tile" style="width:26px;height:26px;background:linear-gradient(158deg,#4f8ae0,#2f6fd0);box-shadow:0 1px 2px rgba(47,111,208,.35)">
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round">
          <circle cx="12" cy="12" r="8.2"></circle><circle cx="12" cy="12" r="2.2"></circle>
        </svg>
      </span>
      <span style="font-size:15px;font-weight:640;letter-spacing:-.018em">DiskMate</span>
    </div>

    <nav style="display:flex;flex-direction:column;gap:2px">
${NAV.map(item).join("\n")}
    </nav>

    <div style="display:flex;flex-direction:column;gap:8px">
      <div class="cap" style="padding:0 8px">This Mac</div>
      <div style="padding:11px 12px 12px;border-radius:12px;background:var(--card);box-shadow:var(--shadow-sm);display:flex;flex-direction:column;gap:9px">
        <div class="row" style="gap:8px">
          <span class="tile" style="width:22px;height:22px;background:var(--sunken)">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="var(--t2)" stroke-width="1.8"><rect x="3.5" y="6" width="17" height="12" rx="2.4"></rect><circle cx="16.6" cy="12" r="1.5" fill="var(--t2)" stroke="none"></circle></svg>
          </span>
          <span style="font-size:13px;font-weight:590;letter-spacing:-.012em">Macintosh HD</span>
        </div>
        <div class="row" style="gap:2px;height:8px;border-radius:999px;overflow:hidden;background:var(--free)">
          <span style="flex:42.6;height:100%;background:var(--c-system)"></span>
          <span style="flex:38.2;height:100%;background:var(--c-applications)"></span>
          <span style="flex:31.4;height:100%;background:var(--c-developer)"></span>
          <span style="flex:28.9;height:100%;background:var(--c-media)"></span>
          <span style="flex:21.7;height:100%;background:var(--c-caches)"></span>
          <span style="flex:16.3;height:100%;background:var(--c-documents)"></span>
          <span style="flex:12.8;height:100%;background:var(--c-downloads)"></span>
          <span style="flex:28.1;height:100%;background:var(--c-other)"></span>
          <span style="flex:25.6;height:100%;background:transparent"></span>
        </div>
        <div class="row num" style="justify-content:space-between;font-size:11.5px;font-weight:600;color:var(--t2)">
          <span>220 GB used</span><span style="color:var(--t3)">25.6 GB free</span>
        </div>
      </div>
    </div>

    <div style="display:flex;flex-direction:column;gap:3px">
      <div class="cap" style="padding:0 8px 4px">Recent</div>
      <div class="row" style="gap:9px;height:28px;padding:0 8px;border-radius:7px">
        <span style="width:6px;height:6px;border-radius:999px;background:var(--c-other);flex:none"></span>
        <span style="font-size:13px;color:var(--t1)">Home</span>
        <span class="num" style="margin-left:auto;font-size:11.5px;color:var(--t3)">118 GB</span>
      </div>
      <div class="row" style="gap:9px;height:28px;padding:0 8px;border-radius:7px">
        <span style="width:6px;height:6px;border-radius:999px;background:var(--c-other);flex:none"></span>
        <span style="font-size:13px;color:var(--t1)">Xcode</span>
        <span class="num" style="margin-left:auto;font-size:11.5px;color:var(--t3)">34.1 GB</span>
      </div>
    </div>

    <div class="row" style="margin-top:auto;gap:7px;padding:0 8px 2px">
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="var(--t3)" stroke-width="1.8" stroke-linecap="round"><rect x="5" y="10.5" width="14" height="9.5" rx="2.4"></rect><path d="M8.4 10.5V8a3.6 3.6 0 0 1 7.2 0v2.5"></path></svg>
      <span style="font-size:11.5px;font-weight:500;color:var(--t3);letter-spacing:-.005em">Nothing leaves this Mac</span>
    </div>
  </aside>
`);
