// Stands in for the Tauri backend so the real frontend can run in a browser.
// Every call is logged, so a gesture that should reach the store but does not
// is visible in the console.
const days = [];
for (let i = 0; i < 7; i++) {
  const d = new Date(2026, 8, 7 + i);
  days.push({
    date: `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,"0")}-${String(d.getDate()).padStart(2,"0")}`,
    placements: [],
  });
}
const week = { anchor: "2026-09-07", viewing_tz: "America/Chicago", days, diagnostics: [] };
window.__calls = [];
window.__TAURI__ = {
  core: {
    invoke: async (cmd, args) => {
      window.__calls.push({ cmd, args });
      console.log("[INVOKE]", cmd, JSON.stringify(args || {}));
      switch (cmd) {
        case "cmd_get_week": return week;
        case "cmd_get_items": return [];
        case "cmd_active_session": return null;
        case "cmd_stats": return { phrase: null, n: 0 };
        case "cmd_data_version": return 1;
        case "cmd_help": return "help";
        case "cmd_add": return {
          item: { id: "itm_stub", title: args.text, tags: [], category: null, location: null,
                  listed: true, due_at: null, estimate_min: null, recurs: false, done: false,
                  completed_at: null, spent_sec: 0 },
          consumed: ["stub"], byday: [], span: null, location: null,
        };
        default: return null;
      }
    },
  },
  event: { listen: () => {} },
};
window.addEventListener("error", (e) => console.log("[WINDOW ERROR]", e.message, e.filename + ":" + e.lineno));
window.addEventListener("unhandledrejection", (e) =>
  console.log("[UNHANDLED REJECTION]", e.reason && e.reason.stack ? e.reason.stack : String(e.reason)));
