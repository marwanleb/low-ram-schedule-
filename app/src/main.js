import { startSky } from "./sky.js";
import { openDetail, zoneLabel } from "./detail.js";

const invoke = window.__TAURI__.core.invoke;

const CAT_COLOR = {
  work: "#7FD0EC",
  life: "#FFCF95",
  body: "#A8D98F",
  social: "#FF8A76",
};
const DAY_MS = 86400000;
const ROW_H = 46;
const ROW_H_COLLAPSED = 17;

const state = {
  anchor: null,       // Monday of the shown week, "YYYY-MM-DD"
  week: null,
  items: [],
  session: null,      // running timer, or null
  pinDay: null,       // clicked day — sticky filter
  peekDay: null,      // hover-held day — transient, also widens the column
  focus: "schedule",
};

const $ = (id) => document.getElementById(id);

// Handy when poking at the running app from the devtools console.
window.__state = state;

/* ── dates ─────────────────────────────────────────────────────────── */
const iso = (d) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const fromIso = (s) => {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y, m - 1, d);
};
const mondayOf = (d) => {
  const copy = new Date(d);
  copy.setDate(copy.getDate() - ((copy.getDay() + 6) % 7));
  return copy;
};
const WD = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
const MON = ["JAN","FEB","MAR","APR","MAY","JUN","JUL","AUG","SEP","OCT","NOV","DEC"];

function hhmm(rfc) {
  const d = new Date(rfc);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}
function fmtDur(sec) {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
function fmtMins(m) {
  if (m % 60 === 0) return `${m / 60}h`;
  return m > 60 ? `${Math.floor(m / 60)}h${m % 60}m` : `${m}m`;
}

/* ── loading ───────────────────────────────────────────────────────── */
async function refresh() {
  const [week, items, session] = await Promise.all([
    invoke("cmd_get_week", { anchor: state.anchor }),
    invoke("cmd_get_items", { openOnly: false }),
    invoke("cmd_active_session"),
  ]);
  state.week = week;
  state.anchor = week.anchor;
  state.items = items;
  state.session = session;
  render();
  refreshStats();
}

async function refreshStats() {
  const s = await invoke("cmd_stats", { tag: null });
  $("statsLine").textContent = s.phrase
    ? `things take ${s.phrase} than you estimate  ·  ${s.n} timed`
    : s.n === 0
      ? "start a timer on something to learn how long it really takes"
      : `${s.n} timed so far — 5 needed before the estimate figure means anything`;
}

/* ── rendering ─────────────────────────────────────────────────────── */
function render() {
  renderHeader();
  renderDiagnostics();
  renderWeek();
  renderTodos();
  renderTimer();
}

function renderHeader() {
  const mon = fromIso(state.anchor);
  $("weekTitle").textContent = `WEEK OF ${MON[mon.getMonth()]} ${mon.getDate()}`;
  const tz = state.week.viewing_tz || "";
  $("zoneChip").textContent = tz.split("/").pop().replace(/_/g, " ").toUpperCase() || "—";
  $("zoneChip").title = tz;
}

function renderDiagnostics() {
  const box = $("diagnostics");
  const ds = state.week.diagnostics || [];
  box.hidden = ds.length === 0;
  box.innerHTML = "";
  // Never silently drop a rule: if something could not be rendered, say so.
  for (const d of ds) {
    const row = document.createElement("div");
    row.innerHTML = `<span class="tag">NOTE</span>${escapeHtml(d.message)}`;
    box.appendChild(row);
  }
}

/** Hours with nothing on any day of the week collapse, so the useful part of
 *  the day fills the pane. */
function occupiedHours() {
  const used = new Set();
  for (const day of state.week.days) {
    for (const p of day.placements) {
      const a = new Date(p.starts_at).getHours();
      const b = new Date(p.ends_at);
      const endH = b.getMinutes() > 0 ? b.getHours() : b.getHours() - 1;
      for (let h = a; h <= Math.max(a, endH); h++) used.add(h);
    }
  }
  return used;
}

function renderWeek() {
  const todayIso = iso(new Date());
  const heads = $("dayHeads");
  heads.innerHTML = "";

  // Matches the hour gutter below, so the headers sit over their columns.
  const spacer = document.createElement("div");
  spacer.className = "headGutter";
  heads.appendChild(spacer);

  // Exactly one column is wide: the peeked day if there is one, else today.
  const wideDay = state.peekDay || todayIso;

  state.week.days.forEach((day, i) => {
    const b = document.createElement("button");
    b.className = "dayHead";
    if (day.date === todayIso) b.classList.add("today");
    if (i >= 5) b.classList.add("weekend");
    if (state.pinDay === day.date) b.classList.add("sel");
    if (state.peekDay === day.date) b.classList.add("peek");
    if (day.date === wideDay) b.classList.add("wide");
    const d = fromIso(day.date);
    b.innerHTML = `<span class="wd">${WD[i]}</span><span class="dn">${d.getDate()}</span>`;
    b.onclick = (e) => {
      e.stopPropagation();
      state.pinDay = state.pinDay === day.date ? null : day.date;
      focusPane("todos");
      render();
    };
    armPeek(b, day.date);
    heads.appendChild(b);
  });

  const used = occupiedHours();
  const hours = [];
  for (let h = 7; h <= 22; h++) hours.push(h);

  // Empty hours give up only as much height as it takes to fit the pane, and
  // never below the floor. All-or-nothing collapsing squeezed a schedule that
  // had room to spare; if the squeeze is not enough, the grid scrolls instead
  // of crushing the day.
  // The grid's allocated box, not clientHeight: this runs before the new rows
  // are appended, so clientHeight would still be describing the old content.
  // getBoundingClientRect is set by the flex layout and is stable either way.
  const PAD_BOTTOM = 16;
  const available = $("grid").clientHeight - PAD_BOTTOM;
  const empties = hours.filter((h) => !used.has(h)).length;
  const deficit = Math.max(0, hours.length * ROW_H - available);
  const give = empties ? Math.min(ROW_H - ROW_H_COLLAPSED, deficit / empties) : 0;
  // Floor, not round: rounding up puts the total back over the edge and the
  // pane scrolls by a few pixels for no reason.
  const emptyH = Math.max(ROW_H_COLLAPSED, Math.floor(ROW_H - give));
  const heightOf = (h) => (used.has(h) ? ROW_H : emptyH);

  const offsets = {};
  let acc = 0;
  for (const h of hours) {
    offsets[h] = acc;
    acc += heightOf(h);
  }

  const grid = $("grid");
  grid.innerHTML = "";
  const inner = document.createElement("div");
  inner.className = "gridInner";

  const gutter = document.createElement("div");
  gutter.className = "hours";
  for (const h of hours) {
    const c = document.createElement("div");
    // Below about two thirds there is no room for "10 AM".
    const small = heightOf(h) < ROW_H * 0.66;
    c.className = "hourCell" + (small ? " collapsed" : "");
    c.style.height = `${heightOf(h)}px`;
    const ampm = h < 12 ? "AM" : "PM";
    const h12 = h % 12 === 0 ? 12 : h % 12;
    c.textContent = small ? `${h12}${ampm[0].toLowerCase()}` : `${h12} ${ampm}`;
    gutter.appendChild(c);
  }
  inner.appendChild(gutter);

  let blocks = 0;
  state.week.days.forEach((day) => {
    const col = document.createElement("div");
    col.className = "dayCol"
      + (day.date === todayIso ? " today" : "")
      + (state.peekDay === day.date ? " peek" : "")
      + (day.date === wideDay ? " wide" : "");
    armPeek(col, day.date);
    col.style.height = `${acc}px`;
    for (const h of hours) {
      const sp = document.createElement("div");
      sp.className = "spacer";
      sp.style.height = `${heightOf(h)}px`;
      col.appendChild(sp);
    }

    for (const p of day.placements) {
      blocks++;
      const s = new Date(p.starts_at);
      const e = new Date(p.ends_at);
      const startH = s.getHours();
      if (offsets[startH] === undefined) continue;

      const top = offsets[startH] + (s.getMinutes() / 60) * heightOf(startH);
      const mins = Math.max(15, (e - s) / 60000);
      const height = Math.max(18, (mins / 60) * ROW_H);

      const color = CAT_COLOR[p.category] || "#7FD0EC";
      const ev = document.createElement("div");
      ev.className = "ev" + (p.done ? " done" : "");
      ev.style.top = `${top}px`;
      ev.style.height = `${height}px`;
      ev.style.borderLeftColor = color;
      ev.style.background = hexA(color, 0.13);
      ev.style.color = color;

      const zone = p.foreign && p.pinned_tz
        ? `<span class="zone">${escapeHtml(zoneLabel(p.pinned_tz))}</span>`
        : "";
      const rep = p.recurs ? `<span class="rep">&#8635;</span>` : "";
      const place = p.location ? `<span class="place">${escapeHtml(p.location)}</span>` : "";
      ev.innerHTML =
        `<span class="t">${escapeHtml(p.title)}</span>` +
        `<span class="h" style="color:${color}">${hhmm(p.starts_at)}&ndash;${hhmm(p.ends_at)}${rep}${zone}${place}</span>`;

      ev.onclick = (evt) => {
        evt.stopPropagation();
        const item = state.items.find((i) => i.id === p.item_id);
        if (item) openDetail(item, day.date, refresh);
      };
      ev.ondblclick = (evt) => {
        evt.stopPropagation();
        toggleTimer(p.item_id, p.id);
      };
      ev.title = "click to edit · double-click to time it";
      col.appendChild(ev);
    }

    inner.appendChild(col);
  });

  grid.appendChild(inner);

  // The grid scrolls and the header does not, so the header needs the
  // scrollbar's width added to its right padding.
  const sbw = grid.offsetWidth - grid.clientWidth;
  heads.style.paddingRight = `${12 + sbw}px`;

  // One template string, applied to both rows, so the headers and the columns
  // cannot disagree about where a day begins.
  const template = `52px ${state.week.days
    .map((d) => (d.date === wideDay ? "1.5fr" : "1fr"))
    .join(" ")}`;
  heads.style.gridTemplateColumns = template;
  inner.style.gridTemplateColumns = template;

  // Drop targets are measured from the real column rects rather than by
  // dividing the width by seven — the columns are not equal widths.
  state.dropZones = Array.from(inner.querySelectorAll(".dayCol")).map((col, i) => ({
    col,
    date: state.week.days[i].date,
    hours,
    heightOf,
  }));

  $("blockCount").textContent = `${blocks} block${blocks === 1 ? "" : "s"}`;
}

/** Buckets are derived from the due date, never stored — otherwise "today"
 *  goes stale the moment tomorrow arrives. */
function bucketOf(item) {
  if (!item.due_at) return "someday";
  const due = new Date(item.due_at);
  const today = new Date();
  today.setHours(23, 59, 59, 999);
  if (due <= today) return "today";
  const endOfWeek = fromIso(state.anchor);
  endOfWeek.setDate(endOfWeek.getDate() + 7);
  return due < endOfWeek ? "week" : "someday";
}

function renderTodos() {
  const wrap = $("buckets");
  wrap.innerHTML = "";

  // A peeked day filters the list the same way a pinned one does; pinning just
  // makes it stick. Spec: "Day peek and pin".
  const focusDay = state.peekDay || state.pinDay;
  let pool = state.items.filter((i) => i.listed);
  if (focusDay) {
    pool = pool.filter((i) => i.due_at && iso(new Date(i.due_at)) === focusDay);
  }

  const bar = $("filterBar");
  if (focusDay) {
    const d = fromIso(focusDay);
    const wd = WD[(d.getDay() + 6) % 7];
    bar.hidden = false;
    bar.classList.toggle("peek", !state.pinDay);
    $("filterLabel").textContent = state.pinDay
      ? `FILTERED · ${wd}`
      : `PEEKING · ${wd} ${d.getDate()}`;
    // Peeking evaporates on its own, so it offers nothing to clear.
    $("filterClear").hidden = !state.pinDay;
  } else {
    bar.hidden = true;
  }

  const groups = { today: [], week: [], someday: [] };
  for (const i of pool) groups[bucketOf(i)].push(i);

  const labels = { today: "TODAY", week: "THIS WEEK", someday: "SOMEDAY" };
  for (const key of ["today", "week", "someday"]) {
    const list = groups[key];
    const sec = document.createElement("div");
    sec.className = `bucket ${key}`;
    const open = list.filter((i) => !i.done).length;
    sec.innerHTML =
      `<div class="bucketHead"><span class="lbl">${labels[key]}</span>` +
      `<span class="rule"></span><span class="n">${open} open</span></div>`;

    if (list.length === 0) {
      const e = document.createElement("div");
      e.className = "empty";
      e.textContent = focusDay ? "nothing here" : "—";
      sec.appendChild(e);
    }

    for (const item of list) sec.appendChild(taskRow(item));
    wrap.appendChild(sec);
  }

  const open = state.items.filter((i) => i.listed && !i.done).length;
  $("openCount").textContent = `${open} open`;
}

function taskRow(item) {
  const color = CAT_COLOR[item.category] || "#59637a";
  const row = document.createElement("div");
  row.className = "task" + (item.done ? " done" : "");
  row.style.borderLeftColor = item.done ? "#2a3140" : color;

  const box = document.createElement("button");
  box.className = "box";
  box.innerHTML = "<i></i>";
  box.style.borderColor = item.done ? color : "";
  box.style.background = item.done ? color : "";
  box.title = item.recurs ? "tick off this week" : "tick off";
  box.onclick = async (e) => {
    e.stopPropagation();
    await invoke("cmd_set_done", { id: item.id, done: !item.done, on: null });
    await refresh();
  };

  const txt = document.createElement("span");
  txt.className = "txt";
  txt.textContent = item.title;

  const chips = document.createElement("span");
  chips.className = "chips";
  if (item.estimate_min) {
    const over = item.spent_sec > item.estimate_min * 60;
    chips.appendChild(chip(`~${fmtMins(item.estimate_min)}`, over ? "#FF8A76" : "#59637a"));
  }
  if (item.due_at) {
    const d = new Date(item.due_at);
    chips.appendChild(chip(WD[(d.getDay() + 6) % 7], color));
  }
  if (item.recurs) chips.appendChild(chip("↻", color));

  const play = document.createElement("button");
  play.className = "play";
  play.style.color = color;
  const running = state.session && state.session.item_id === item.id;
  play.textContent = running ? "■" : "▶";
  play.title = running ? "stop the timer" : "start a timer";
  play.onclick = (e) => {
    e.stopPropagation();
    toggleTimer(item.id, null);
  };
  chips.appendChild(play);

  txt.onclick = () => openDetail(item, null, refresh);
  txt.style.cursor = "pointer";
  txt.title = "click to edit";

  row.append(box, txt, chips);
  makeDraggable(row, item);
  return row;
}

function chip(text, color) {
  const s = document.createElement("span");
  s.className = "tagChip";
  s.style.color = color;
  s.textContent = text;
  return s;
}

/* ── timer ─────────────────────────────────────────────────────────── */
async function toggleTimer(itemId, placementId) {
  if (state.session && state.session.item_id === itemId) {
    await invoke("cmd_timer_stop", { sessionId: state.session.id });
  } else {
    // Starting a second timer stops the first; the backend reports which.
    await invoke("cmd_timer_start", { itemId, placementId });
  }
  await refresh();
}

$("timerStop").onclick = async (e) => {
  e.stopPropagation();
  if (state.session) await invoke("cmd_timer_stop", { sessionId: state.session.id });
  await refresh();
};

function renderTimer() {
  const bar = $("timerBar");
  if (!state.session) {
    bar.hidden = true;
    return;
  }
  bar.hidden = false;
  $("timerTitle").textContent = state.session.title;
  const est = state.session.estimate_min;
  $("timerEstimate").textContent = est ? `est ${fmtMins(est)}` : "";
  tickTimer();
}

function tickTimer() {
  if (!state.session) return;
  const banked = state.session.spent_sec || 0;
  const live = (Date.now() - new Date(state.session.started_at).getTime()) / 1000;
  const total = banked + Math.max(0, live);
  $("timerElapsed").textContent = fmtDur(total);
  const est = state.session.estimate_min;
  $("timerEstimate").classList.toggle("over", !!est && total > est * 60);
}
// The clock is rendered locally; nothing polls the backend for it.
setInterval(tickTimer, 1000);

/* ── input ─────────────────────────────────────────────────────────── */
$("addForm").onsubmit = async (e) => {
  e.preventDefault();
  const input = $("addInput");
  const text = input.value.trim();
  if (!text) return;

  if (/^\/?(help|\?|h)$/i.test(text)) {
    await showHelp();
    input.value = "";
    return;
  }
  const res = await invoke("cmd_add", { text });
  input.value = "";
  await refresh();
  showEcho(res);
};

let echoTimer = null;
/** Say what was understood. A line the parser did not recognise is saved whole
 *  — which is correct, but silent, and silence is how a typo goes unnoticed. */
function showEcho(res) {
  const box = $("addEcho");
  clearTimeout(echoTimer);

  if (!res.consumed.length) {
    box.className = "addEcho plain";
    box.innerHTML =
      `saved as plain text — nothing recognised in <b>${escapeHtml(res.item.title)}</b>. type <b>help</b> for the syntax`;
  } else {
    const bits = [];
    if (res.byday.length) bits.push(`repeats ${res.byday.join(" ")}`);
    if (res.span) bits.push(`${res.span[0]}–${res.span[1]}`);
    if (res.item.due_at && !res.byday.length) {
      bits.push(`due ${new Date(res.item.due_at).toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" })}`);
    }
    if (res.item.estimate_min) bits.push(`~${fmtMins(res.item.estimate_min)}`);
    if (res.location) bits.push(res.location);
    box.className = "addEcho";
    box.innerHTML = `<b>${escapeHtml(res.item.title)}</b> · ${escapeHtml(bits.join(" · "))}`;
  }

  box.hidden = false;
  echoTimer = setTimeout(() => (box.hidden = true), 6000);
}

async function showHelp() {
  $("helpBody").textContent = await invoke("cmd_help");
  $("helpOverlay").hidden = false;
}
$("detailClose").onclick = () => ($("detailOverlay").hidden = true);
$("detailOverlay").onclick = (e) => {
  if (e.target === $("detailOverlay")) $("detailOverlay").hidden = true;
};
$("helpClose").onclick = () => ($("helpOverlay").hidden = true);
$("helpOverlay").onclick = (e) => {
  if (e.target === $("helpOverlay")) $("helpOverlay").hidden = true;
};

function focusPane(which) {
  state.focus = which;
  document.body.classList.toggle("todoFocus", which === "todos");
  $("schedulePane").classList.toggle("focused", which === "schedule");
  $("todoPane").classList.toggle("focused", which === "todos");
}
$("schedulePane").onclick = () => { $("weekPicker").hidden = true; focusPane("schedule"); };
$("todoPane").onclick = () => { $("weekPicker").hidden = true; focusPane("todos"); };

function shiftWeek(delta) {
  const d = fromIso(state.anchor);
  d.setDate(d.getDate() + delta * 7);
  state.anchor = iso(d);
  refresh();
}
$("filterClear").onclick = (e) => {
  e.stopPropagation();
  state.pinDay = null;
  render();
};
$("prevWeek").onclick = (e) => { e.stopPropagation(); shiftWeek(-1); };
$("nextWeek").onclick = (e) => { e.stopPropagation(); shiftWeek(1); };
/** Clicking through a week at a time to reach something three months out is
 *  no way to plan; the picker jumps straight there. */
function renderPicker() {
  const rows = $("pickRows");
  rows.innerHTML = "";
  const thisMonday = mondayOf(new Date());
  const shown = fromIso(state.anchor);

  for (let off = -4; off <= 28; off++) {
    const start = new Date(thisMonday);
    start.setDate(start.getDate() + off * 7);
    const end = new Date(start);
    end.setDate(end.getDate() + 6);

    const b = document.createElement("button");
    b.className = "pickRow";
    if (off === 0) b.classList.add("now");
    if (iso(start) === iso(shown)) b.classList.add("sel");

    const fmt = (d) => `${MON[d.getMonth()]} ${d.getDate()}`;
    const rel =
      off === 0 ? "THIS WEEK"
      : off === 1 ? "NEXT WEEK"
      : off === -1 ? "LAST WEEK"
      : off > 0 ? `+${off} WKS` : `${off} WKS`;

    b.innerHTML =
      `<span class="range">${fmt(start)} – ${fmt(end)}</span><span class="rel">${rel}</span>`;
    b.onclick = (e) => {
      e.stopPropagation();
      state.anchor = iso(start);
      state.pinDay = null;
      $("weekPicker").hidden = true;
      refresh();
    };
    rows.appendChild(b);
  }
}

$("today").onclick = (e) => {
  e.stopPropagation();
  const picker = $("weekPicker");
  if (picker.hidden) {
    renderPicker();
    picker.hidden = false;
    // Open on the week being shown, not scrolled to the top.
    const sel = picker.querySelector(".pickRow.sel");
    if (sel) sel.scrollIntoView({ block: "center" });
  } else {
    picker.hidden = true;
  }
};

$("hide").onclick = (e) => { e.stopPropagation(); invoke("cmd_hide"); };

// Row heights are fitted to the pane, so a resize has to re-fit them.
// Debounced, because a drag fires this continuously.
let resizeTimer = null;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => {
    if (state.week) render();
  }, 120);
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (!$("helpOverlay").hidden) { $("helpOverlay").hidden = true; return; }
    if (!$("detailOverlay").hidden) { $("detailOverlay").hidden = true; return; }
    if (!$("weekPicker").hidden) { $("weekPicker").hidden = true; return; }
    if (document.activeElement === $("addInput")) { $("addInput").blur(); return; }
    invoke("cmd_hide");
  }
  if (e.key === "/" && document.activeElement !== $("addInput")) {
    e.preventDefault();
    $("addInput").focus();
  }
});

/* ── day peek ───────────────────────────────────────────────────────── */
let peekTimer = null;

/** A second of hover widens the day and previews its to-dos. Deliberately slow
 *  so brushing past the grid does not make it jump about. */
function armPeek(node, date) {
  node.addEventListener("pointerenter", () => {
    clearTimeout(peekTimer);
    peekTimer = setTimeout(() => {
      if (state.peekDay === date) return;
      state.peekDay = date;
      render();
    }, 1000);
  });
  node.addEventListener("pointerleave", () => {
    clearTimeout(peekTimer);
    if (state.peekDay === date) {
      state.peekDay = null;
      render();
    }
  });
}

/* ── drag a task onto the week ──────────────────────────────────────── */
/** Dropping creates a placement for the SAME item, never a copy. Spec 3.1. */
function makeDraggable(row, item) {
  row.style.touchAction = "none";
  row.addEventListener("pointerdown", (e) => {
    // The checkbox and the timer control own their own clicks.
    if (e.target.closest(".box, .play")) return;

    const startX = e.clientX;
    const startY = e.clientY;
    let dragging = false;
    const ghost = $("dragGhost");

    const move = (ev) => {
      if (!dragging) {
        // A few pixels of slop, so a click stays a click.
        if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < 5) return;
        dragging = true;
        ghost.hidden = false;
        ghost.textContent = item.title;
        ghost.style.borderColor = CAT_COLOR[item.category] || "#59637a";
      }
      ghost.style.left = `${ev.clientX}px`;
      ghost.style.top = `${ev.clientY}px`;
      highlight(hitTest(ev.clientX, ev.clientY));
    };

    const up = async (ev) => {
      try { row.releasePointerCapture?.(e.pointerId); } catch { /* never captured */ }
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      ghost.hidden = true;
      highlight(null);
      if (!dragging) return;

      const hit = hitTest(ev.clientX, ev.clientY);
      if (!hit) return;
      const starts = new Date(`${hit.date}T${String(hit.hour).padStart(2, "0")}:00:00`);
      const mins = item.estimate_min && item.estimate_min <= 480 ? item.estimate_min : 60;
      const ends = new Date(starts.getTime() + mins * 60000);
      try {
        await invoke("cmd_add_placement", {
          itemId: item.id,
          startsAt: rfc(starts),
          endsAt: rfc(ends),
        });
        await refresh();
      } catch (err) {
        showError(String(err));
      }
    };

    // Listeners first: setPointerCapture throws for a pointer the browser is
    // not tracking, and if that ran first the drag would never arm.
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    try {
      row.setPointerCapture?.(e.pointerId);
    } catch {
      // Capture is an optimisation; the window listeners do the real work.
    }
  });
}

/** Which day column and hour the cursor is over, by real geometry. */
function hitTest(x, y) {
  for (const z of state.dropZones || []) {
    const r = z.col.getBoundingClientRect();
    if (x < r.left || x > r.right || y < r.top || y > r.bottom) continue;
    let acc = r.top;
    for (const h of z.hours) {
      const height = z.heightOf(h);
      if (y >= acc && y < acc + height) return { date: z.date, hour: h, col: z.col };
      acc += height;
    }
    return { date: z.date, hour: z.hours[z.hours.length - 1], col: z.col };
  }
  return null;
}

let lastHighlight = null;
function highlight(hit) {
  if (lastHighlight && lastHighlight !== hit?.col) lastHighlight.classList.remove("dropTarget");
  if (hit) hit.col.classList.add("dropTarget");
  lastHighlight = hit?.col || null;
}

function rfc(d) {
  const pad = (n) => String(n).padStart(2, "0");
  const off = -d.getTimezoneOffset();
  const sign = off >= 0 ? "+" : "-";
  const a = Math.abs(off);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T` +
    `${pad(d.getHours())}:${pad(d.getMinutes())}:00${sign}${pad(Math.floor(a / 60))}:${pad(a % 60)}`;
}

function showError(msg) {
  const box = $("diagnostics");
  box.hidden = false;
  box.innerHTML = `<div><span class="tag">ERROR</span>${escapeHtml(msg)}</div>`;
}

/* ── helpers ───────────────────────────────────────────────────────── */
function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}
function hexA(hex, a) {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
}

/* ── go ────────────────────────────────────────────────────────────── */
state.anchor = iso(mondayOf(new Date()));
startSky(document.getElementById("sky"));
refresh().catch((e) => {
  document.body.insertAdjacentHTML(
    "afterbegin",
    `<div id="diagnostics"><span class="tag">ERROR</span>${escapeHtml(String(e))}</div>`
  );
});
