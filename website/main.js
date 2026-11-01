// RustLauncherMC's website: the pixel sky, the launcher preview, the latest release, and small
// page behaviour. Plain JavaScript, no dependencies.
"use strict";

const REPO = "RustLauncherMC/Launcher";
const RELEASES = `https://github.com/${REPO}/releases`;
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

document.documentElement.classList.add("js");

/* ---------- Navigation ---------- */
const nav = document.querySelector(".nav");
const onScroll = () => nav.classList.toggle("scrolled", window.scrollY > 8);
window.addEventListener("scroll", onScroll, { passive: true });
onScroll();

const toggle = document.querySelector(".nav-toggle");
const links = document.getElementById("nav-links");
toggle.addEventListener("click", () => {
  const open = links.classList.toggle("open");
  toggle.setAttribute("aria-expanded", String(open));
});
links.addEventListener("click", (e) => {
  if (e.target.closest("a")) {
    links.classList.remove("open");
    toggle.setAttribute("aria-expanded", "false");
  }
});

/* ---------- Reveal on scroll ---------- */
const reveals = document.querySelectorAll(".reveal");
if ("IntersectionObserver" in window && !reducedMotion) {
  const seen = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("shown");
          seen.unobserve(entry.target);
        }
      }
    },
    { rootMargin: "0px 0px -8% 0px" }
  );
  reveals.forEach((el) => seen.observe(el));
} else {
  reveals.forEach((el) => el.classList.add("shown"));
}

/* ---------- Pixel art ---------- */
// A small deterministic random number generator, so the scene is the same on every load.
function rng(seed) {
  return () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 4294967296;
  };
}

// Smooth 1D noise from a seeded table, for hills.
function noise1d(seed) {
  const r = rng(seed);
  const table = Array.from({ length: 64 }, r);
  return (x) => {
    const i = Math.floor(x);
    const f = x - i;
    const a = table[((i % 64) + 64) % 64];
    const b = table[(((i + 1) % 64) + 64) % 64];
    const t = f * f * (3 - 2 * f);
    return a + (b - a) * t;
  };
}

// Draws at a quarter of the screen's resolution; CSS scales it up without smoothing.
const PIXEL = 4;

function setupCanvas(canvas) {
  const ctx = canvas.getContext("2d");
  const size = () => {
    const w = Math.ceil(canvas.clientWidth / PIXEL);
    const h = Math.ceil(canvas.clientHeight / PIXEL);
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
      return true;
    }
    return false;
  };
  return { ctx, size };
}

/* The hero: a night sky going into a sunset, a blocky sun, clouds, two ranges of blocky hills
   with trees, and twinkling stars. */
function hero(canvas) {
  const { ctx, size } = setupCanvas(canvas);
  const hillsFar = noise1d(7);
  const hillsNear = noise1d(23);
  let stars = [];
  let clouds = [];

  const build = () => {
    const r = rng(42);
    const { width: w, height: h } = canvas;
    stars = Array.from({ length: Math.round((w * h) / 260) }, () => ({
      x: Math.floor(r() * w),
      y: Math.floor(r() * (h - 260 / PIXEL)),
      p: r() * Math.PI * 2,
      s: 0.5 + r() * 1.5,
      big: r() < 0.06,
    }));
    clouds = Array.from({ length: 6 }, (_, i) => ({
      x: r() * w,
      y: h - (150 + r() * 90) / PIXEL,
      w: 18 + Math.floor(r() * 28),
      speed: 0.02 + r() * 0.03,
      layer: i % 2,
    }));
  };

  const draw = (time) => {
    if (size()) build();
    const { width: w, height: h } = canvas;
    // The scene sits at the bottom of the hero whatever its height, under the text.
    const horizon = h - Math.round(110 / PIXEL);

    // Sky.
    const sky = ctx.createLinearGradient(0, 0, 0, horizon);
    sky.addColorStop(0, "#100a1f");
    const at = (px) => Math.min(0.99, Math.max(0.01, 1 - px / PIXEL / horizon));
    sky.addColorStop(at(520), "#1d1340");
    sky.addColorStop(at(260), "#4a1f5c");
    sky.addColorStop(at(110), "#a63a5a");
    sky.addColorStop(1, "#f0795a");
    ctx.fillStyle = sky;
    ctx.fillRect(0, 0, w, h);

    // Stars.
    for (const s of stars) {
      const a = reducedMotion ? 0.7 : 0.35 + 0.65 * Math.abs(Math.sin(time / 1000 * s.s + s.p));
      const fade = Math.max(0, 1 - s.y / (h - 200 / PIXEL));
      ctx.fillStyle = `rgba(244, 241, 255, ${(a * fade).toFixed(3)})`;
      ctx.fillRect(s.x, s.y, 1, 1);
      if (s.big && a > 0.8) {
        ctx.fillStyle = `rgba(244, 241, 255, ${(0.35 * fade).toFixed(3)})`;
        ctx.fillRect(s.x - 1, s.y, 3, 1);
        ctx.fillRect(s.x, s.y - 1, 1, 3);
      }
    }

    // The sun: a pixel disc with gaps near the bottom, half below the horizon.
    const sunR = Math.round(Math.min(110, w * PIXEL * 0.12) / PIXEL);
    const sunX = Math.floor(w * (w * PIXEL > 960 ? 0.7 : 0.62));
    const sunY = horizon - Math.floor(sunR * 0.35);
    for (let y = -sunR; y <= sunR; y++) {
      const half = Math.floor(Math.sqrt(sunR * sunR - y * y));
      const t = (y + sunR) / (2 * sunR);
      if (t > 0.55 && Math.floor(y) % 4 === 0) continue; // the stripes
      const g = Math.round(230 - t * 110);
      ctx.fillStyle = `rgb(255, ${g}, ${Math.round(140 - t * 80)})`;
      ctx.fillRect(sunX - half, sunY + y, half * 2, 1);
    }

    // Clouds, drifting.
    for (const c of clouds) {
      const x = ((c.x + (reducedMotion ? 0 : time * c.speed * 0.06)) % (w + c.w * 2)) - c.w;
      const y = Math.floor(c.y);
      ctx.fillStyle = c.layer ? "rgba(255, 170, 150, 0.55)" : "rgba(214, 120, 160, 0.5)";
      ctx.fillRect(Math.floor(x), y, c.w, 2);
      ctx.fillRect(Math.floor(x + c.w * 0.2), y - 2, Math.floor(c.w * 0.5), 2);
      ctx.fillRect(Math.floor(x + c.w * 0.35), y - 4, Math.floor(c.w * 0.22), 2);
    }

    // Far hills: blocks of 3 pixels, in dusky purple.
    const block = 3;
    ctx.fillStyle = "#2a1a4a";
    for (let x = 0; x < w; x += block) {
      const n = hillsFar(x / 40);
      const top = horizon - 6 - Math.floor((n * 20) / block) * block;
      ctx.fillRect(x, top, block, h - top);
    }
    // Near hills with trees, darker.
    ctx.fillStyle = "#170e2e";
    const r = rng(99);
    for (let x = 0; x < w; x += block) {
      const n = hillsNear(x / 28 + 3);
      const top = horizon + 4 - Math.floor((n * 18) / block) * block;
      ctx.fillRect(x, top, block, h - top);
      if (r() < 0.07) {
        // A tree: a trunk and a blocky crown.
        ctx.fillRect(x + 1, top - 5, 1, 5);
        ctx.fillRect(x - 1, top - 9, 5, 4);
        ctx.fillRect(x, top - 11, 3, 2);
      }
    }

    // Fade into the page.
    const from = h - 60 / PIXEL;
    const fade = ctx.createLinearGradient(0, from, 0, h);
    fade.addColorStop(0, "rgba(16, 10, 31, 0)");
    fade.addColorStop(1, "#100a1f");
    ctx.fillStyle = fade;
    ctx.fillRect(0, from, w, h - from);
  };

  animate(canvas, draw);
}

/* The community section: pixelated aurora bands and a few stars. */
function aurora(canvas) {
  const { ctx, size } = setupCanvas(canvas);
  const draw = (time) => {
    size();
    const { width: w, height: h } = canvas;
    ctx.clearRect(0, 0, w, h);
    const t = reducedMotion ? 0 : time / 4000;
    const bands = [
      { c: "139, 108, 255", y: 0.5, a: 0.22, f: 0.035 },
      { c: "211, 58, 63", y: 0.62, a: 0.16, f: 0.05 },
      { c: "240, 121, 90", y: 0.72, a: 0.1, f: 0.07 },
    ];
    for (const b of bands) {
      for (let x = 0; x < w; x += 2) {
        const wave = Math.sin(x * b.f + t * (1 + b.f * 10)) * h * 0.06 + Math.sin(x * b.f * 0.37 - t) * h * 0.04;
        const top = Math.floor(h * b.y + wave);
        const height = Math.floor(h * 0.18);
        for (let y = 0; y < height; y += 2) {
          const alpha = b.a * Math.sin((y / height) * Math.PI);
          ctx.fillStyle = `rgba(${b.c}, ${alpha.toFixed(3)})`;
          ctx.fillRect(x, top - height / 2 + y, 2, 2);
        }
      }
    }
  };
  animate(canvas, draw);
}

// Runs `draw` every frame while the canvas is on screen (once with reduced motion).
function animate(canvas, draw) {
  let visible = true;
  let frame = 0;
  const loop = (time) => {
    draw(time);
    if (!reducedMotion && visible) frame = requestAnimationFrame(loop);
  };
  if ("IntersectionObserver" in window) {
    new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      cancelAnimationFrame(frame);
      if (visible) frame = requestAnimationFrame(loop);
    }).observe(canvas);
  }
  window.addEventListener("resize", () => draw(performance.now()));
  frame = requestAnimationFrame(loop);
}

hero(document.getElementById("sky"));
aurora(document.getElementById("aurora"));

/* ---------- The launcher preview ---------- */
const editions = {
  java: { title: "JAVA EDITION", sub: "Horizon", name: "Horizon", tag: "Minecraft: Java Edition rewritten in Rust", status: "Latest nightly" },
  bedrock: { title: "BEDROCK EDITION", sub: "Cinnabar", name: "Cinnabar", tag: "A Bedrock Edition client written in Rust", status: "Latest Beta" },
};
const win = (name) => document.querySelector(`[data-win-${name}]`);
let playing = null;

document.querySelectorAll(".win-ed").forEach((button) => {
  button.addEventListener("click", () => {
    const edition = editions[button.dataset.edition];
    document.querySelectorAll(".win-ed").forEach((b) => b.classList.toggle("active", b === button));
    win("title").textContent = edition.title;
    win("sub").textContent = edition.sub;
    win("name").textContent = edition.name;
    win("tag").textContent = edition.tag;
    win("status").textContent = edition.status;
    win("art").classList.toggle("java", button.dataset.edition === "java");
  });
});

win("play").addEventListener("click", () => {
  const play = win("play");
  if (playing) return;
  play.classList.add("busy");
  play.textContent = "STARTING";
  win("status").textContent = "Starting the game…";
  playing = setTimeout(() => {
    play.textContent = "PLAYING";
    win("status").textContent = "Have fun!";
    playing = setTimeout(() => {
      play.classList.remove("busy");
      play.textContent = "PLAY";
      const active = document.querySelector(".win-ed.active").dataset.edition;
      win("status").textContent = editions[active].status;
      playing = null;
    }, 1800);
  }, 1200);
});

/* ---------- Downloads ---------- */
function detectOs() {
  const platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  const ua = navigator.userAgent;
  if (/android|iphone|ipad/i.test(ua)) return "other";
  if (/win/i.test(platform) || /Windows/.test(ua)) return "windows";
  if (/linux/i.test(platform) || /Linux|X11/.test(ua)) return "linux";
  return "other";
}

function megabytes(bytes) {
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

async function showRelease() {
  const os = detectOs();
  const main = document.getElementById("download-main");
  const label = main.querySelector("[data-download-label]");
  const line = document.querySelector("[data-release-line]");
  const note = document.querySelector("[data-download-note]");
  const cards = [...document.querySelectorAll(".dl[data-asset]")];
  const recommended = cards.find((c) => c.dataset.os === os);
  if (recommended) recommended.classList.add("recommended");

  let release = null;
  try {
    const response = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (response.ok) release = await response.json();
  } catch (_) {
    // Offline or blocked: the links still lead to the releases page.
  }

  if (!release || !Array.isArray(release.assets)) {
    line.textContent = "First release coming soon";
    note.textContent = "The first release is on its way. Until then, you can build the launcher from source.";
    main.href = "#download";
    return;
  }

  const version = release.tag_name;
  const pill = document.querySelector("[data-version-pill]");
  pill.textContent = version;
  pill.hidden = false;

  const find = (pattern) => {
    if (pattern.includes("VERSION")) {
      const [start, end] = pattern.split("VERSION");
      return release.assets.find((a) => a.name.startsWith(start) && a.name.endsWith(end));
    }
    return release.assets.find((a) => a.name === pattern);
  };
  for (const card of cards) {
    const asset = find(card.dataset.asset);
    if (asset) card.href = asset.browser_download_url;
  }

  const mine = recommended && find(recommended.dataset.asset);
  if (mine) {
    main.href = mine.browser_download_url;
    label.textContent = os === "windows" ? "Download for Windows" : "Download for Linux";
    line.textContent = `Launcher ${version} · ${megabytes(mine.size)}`;
  } else {
    main.href = "#download";
    line.textContent = `Launcher ${version}`;
    if (os === "other") {
      note.textContent = "RustLauncherMC is made for Windows and Linux. Pick the file for the computer you'll play on.";
    }
  }
}

showRelease();
