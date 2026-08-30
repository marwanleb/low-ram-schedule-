/**
 * Pixel-art sky and ocean behind the panes, following the real time of day.
 *
 * Two things matter beyond the look:
 *  - It runs at ~3.3fps deliberately, so motion reads as pixel animation.
 *  - It stops completely when the window is hidden. The app lives in the tray
 *    and this must not burn CPU behind a game.
 */

const PX = 7; // CSS pixels per art pixel
const WATERLINE = 0.78;

// zenith, horizon, light — interpolated across the day.
const DAY = [
  [0.0,  "#070b16", "#0d1424", "#243049"],
  [4.5,  "#0a1020", "#16223a", "#2d3a58"],
  [6.2,  "#1b2545", "#7a5a6b", "#c98a6a"],
  [8.0,  "#2f5f96", "#8fb6cf", "#ffd9a0"],
  [12.0, "#3f86c4", "#a9cfe4", "#fff3cf"],
  [16.5, "#3a7ab5", "#b0c9dd", "#ffe0ad"],
  [19.0, "#274a7e", "#c9805f", "#ff9d5c"],
  [20.6, "#152a52", "#6a3f58", "#a45a6a"],
  [22.0, "#0a1128", "#1b2340", "#3a4668"],
  [24.0, "#070b16", "#0d1424", "#243049"],
];

const BAYER = [
  [0, 8, 2, 10],
  [12, 4, 14, 6],
  [3, 11, 1, 9],
  [15, 7, 13, 5],
];

const hex = (h) => [
  parseInt(h.slice(1, 3), 16),
  parseInt(h.slice(3, 5), 16),
  parseInt(h.slice(5, 7), 16),
];
const mix = (a, b, t) => a.map((v, i) => Math.round(v + (b[i] - v) * t));
const css = (c) => `rgb(${c[0]},${c[1]},${c[2]})`;

function palette(hour) {
  let lo = DAY[0];
  let hi = DAY[DAY.length - 1];
  for (let i = 0; i < DAY.length - 1; i++) {
    if (hour >= DAY[i][0] && hour <= DAY[i + 1][0]) {
      lo = DAY[i];
      hi = DAY[i + 1];
      break;
    }
  }
  const t = (hour - lo[0]) / Math.max(0.0001, hi[0] - lo[0]);
  return {
    zenith: mix(hex(lo[1]), hex(hi[1]), t),
    horizon: mix(hex(lo[2]), hex(hi[2]), t),
    light: mix(hex(lo[3]), hex(hi[3]), t),
  };
}

/** Ordered dither between two palette steps — what makes the fade pixelated. */
function ditherBand(ctx, x, y, top, bottom, t, steps) {
  const level = t * (steps - 1);
  const lo = Math.floor(level);
  const frac = level - lo;
  const a = mix(top, bottom, lo / (steps - 1));
  const b = mix(top, bottom, Math.min(steps - 1, lo + 1) / (steps - 1));
  const threshold = BAYER[y % 4][x % 4] / 16;
  ctx.fillStyle = css(frac > threshold ? b : a);
  ctx.fillRect(x, y, 1, 1);
}

export function startSky(canvas) {
  const ctx = canvas.getContext("2d", { alpha: false });
  let W = 0;
  let H = 0;
  let frame = 0;
  let baked = null;
  let timer = null;
  let raf = null;

  const stars = Array.from({ length: 90 }, (_, i) => ({
    x: (i * 37.7) % 1,
    y: (i * 13.3) % 0.6,
    p: (i * 0.7) % 6.28,
  }));
  const fish = Array.from({ length: 6 }, (_, i) => ({
    x: (i * 0.19) % 1,
    y: 0.82 + ((i * 0.031) % 0.14),
    v: 0.0012 + (i % 3) * 0.0007,
    c: ["#FFCF95", "#7FD0EC", "#FF8A76", "#C79BF0", "#8FD6A8", "#F5B942"][i],
  }));
  const clouds = Array.from({ length: 4 }, (_, i) => ({
    x: (i * 0.28) % 1,
    y: 0.1 + ((i * 0.07) % 0.28),
    w: 9 + (i % 3) * 5,
    v: 0.0006 + (i % 3) * 0.0004,
  }));

  function resize() {
    W = Math.ceil(window.innerWidth / PX);
    H = Math.ceil(window.innerHeight / PX);
    canvas.width = W;
    canvas.height = H;
    canvas.style.width = `${window.innerWidth}px`;
    canvas.style.height = `${window.innerHeight}px`;
    baked = null;
  }

  function bake(pal) {
    const wl = Math.floor(H * WATERLINE);
    const deep = mix(pal.horizon, [4, 10, 22], 0.72);
    const sand = mix(pal.light, [70, 60, 40], 0.62);
    const seabed = Math.floor(wl + (H - wl) * 0.8);

    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        if (y < wl) {
          ditherBand(ctx, x, y, pal.zenith, pal.horizon, y / wl, 8);
        } else if (y < seabed) {
          ditherBand(ctx, x, y, pal.horizon, deep, (y - wl) / (seabed - wl), 14);
        } else {
          ditherBand(ctx, x, y, deep, sand, (y - seabed) / Math.max(1, H - seabed), 7);
        }
      }
    }
    baked = ctx.getImageData(0, 0, W, H);
  }

  function draw() {
    const now = new Date();
    const hour = now.getHours() + now.getMinutes() / 60;
    const pal = palette(hour);
    const wl = Math.floor(H * WATERLINE);

    if (!baked || frame % 20 === 0) bake(pal);
    else ctx.putImageData(baked, 0, 0);

    // sun / moon on a sine altitude curve
    const dayT = (hour - 6) / 14;
    if (dayT >= 0 && dayT <= 1) {
      const sx = Math.floor(dayT * W);
      const sy = Math.floor(wl - Math.sin(dayT * Math.PI) * wl * 0.82);
      const glow = css(pal.light);
      for (let r = 5; r >= 0; r--) {
        ctx.fillStyle = r === 0 ? "#fff6d8" : glow;
        ctx.globalAlpha = r === 0 ? 1 : 0.1;
        ctx.fillRect(sx - r, sy - r, r * 2 + 1, r * 2 + 1);
      }
      ctx.globalAlpha = 1;
    } else {
      const nt = hour > 20 ? (hour - 20) / 10 : (hour + 4) / 10;
      const mx = Math.floor(nt * W);
      const my = Math.floor(wl - Math.sin(nt * Math.PI) * wl * 0.7);
      ctx.fillStyle = "#dfe6f2";
      ctx.fillRect(mx - 3, my - 3, 7, 7);
      ctx.fillStyle = "#c2cbdb";
      ctx.fillRect(mx - 1, my - 1, 2, 2);
    }

    // stars fade in late and out early
    const night = hour > 21.5 || hour < 6.2;
    if (night) {
      for (const s of stars) {
        const tw = 0.5 + 0.5 * Math.sin(frame * 0.4 + s.p);
        ctx.fillStyle = `rgba(233,236,241,${0.25 + tw * 0.55})`;
        ctx.fillRect(Math.floor(s.x * W), Math.floor(s.y * wl), 1, 1);
      }
    }

    // clouds drift right and wrap
    for (const c of clouds) {
      c.x = (c.x + c.v) % 1.2;
      const cx = Math.floor(c.x * W) - 10;
      const cy = Math.floor(c.y * wl);
      ctx.fillStyle = `rgba(${pal.light[0]},${pal.light[1]},${pal.light[2]},0.5)`;
      for (let i = 0; i < 3; i++) {
        ctx.fillRect(cx + i * Math.floor(c.w / 3), cy - (i === 1 ? 2 : 0), c.w / 2, 3);
      }
    }

    // waterline crest from two summed sines
    for (let x = 0; x < W; x++) {
      const wave = Math.sin(x * 0.18 + frame * 0.25) + Math.sin(x * 0.07 - frame * 0.15);
      const y = wl + Math.round(wave);
      ctx.fillStyle = css(mix(pal.light, [255, 255, 255], 0.35));
      ctx.fillRect(x, y, 1, 1);
    }

    // fish
    for (const f of fish) {
      f.x = (f.x + f.v) % 1.1;
      const fx = Math.floor(f.x * W);
      const fy = Math.floor(f.y * H + Math.sin(frame * 0.3 + f.x * 10) * 1.2);
      ctx.fillStyle = f.c;
      ctx.fillRect(fx, fy, 4, 2);
      ctx.fillRect(fx - 1, fy + (frame % 2 === 0 ? -1 : 1), 1, 1);
    }

    frame++;
  }

  function loop() {
    draw();
    timer = setTimeout(() => {
      raf = requestAnimationFrame(loop);
    }, 300);
  }

  function stop() {
    clearTimeout(timer);
    cancelAnimationFrame(raf);
    timer = null;
    raf = null;
  }

  function start() {
    if (timer || raf) return;
    loop();
  }

  // The window spends most of its life in the tray, and a hidden native window
  // does not reliably fire `visibilitychange` — so the Rust side says outright
  // when it hides and shows. `visibilitychange` stays as a second belt.
  const ev = window.__TAURI__?.event;
  if (ev) {
    ev.listen("window:hidden", stop);
    ev.listen("window:shown", () => { resize(); start(); });
  }
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) stop();
    else start();
  });
  window.addEventListener("resize", resize);

  // Exposed so the console can confirm the loop really stops.
  window.__sky = { running: () => timer !== null || raf !== null, stop, start };

  resize();
  start();
}
