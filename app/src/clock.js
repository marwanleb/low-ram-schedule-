/* Clock times as people read them: 12-hour and compact. Display only -- the
   backend, the ISO strings and what you type all stay 24-hour. The Rust side
   has the same rules in core/src/clock.rs. */

/** 15, 30 -> "3:30pm"; 15, 0 -> "3pm"; 12, 0 -> "12pm"; 0, 0 -> "12am". */
export function time12(h, m) {
  const meridiem = h < 12 ? "am" : "pm";
  const h12 = h % 12 === 0 ? 12 : h % 12;
  return m === 0 ? `${h12}${meridiem}` : `${h12}:${String(m).padStart(2, "0")}${meridiem}`;
}

/** "2–5pm", "10:30am–12pm": the meridiem once when both ends share it, which
 *  keeps a range short enough for a narrow block. */
export function range12(h1, m1, h2, m2) {
  const a = time12(h1, m1);
  const b = time12(h2, m2);
  return (h1 < 12) === (h2 < 12) ? `${a.slice(0, -2)}–${b}` : `${a}–${b}`;
}

/** A range from two "HH:MM" strings, the way the backend stores rule times. */
export function rangeHHMM(start, end) {
  const [h1, m1] = start.split(":").map(Number);
  const [h2, m2] = end.split(":").map(Number);
  return range12(h1, m1, h2, m2);
}

/** A range from two instants, read in local time. */
export function rangeOf(startIso, endIso) {
  const a = new Date(startIso);
  const b = new Date(endIso);
  return range12(a.getHours(), a.getMinutes(), b.getHours(), b.getMinutes());
}
