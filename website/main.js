// RustLauncherMC's website: the pixel cog and embers, the launcher preview, the latest release,
// and small page behaviour. Plain JavaScript, no dependencies.
"use strict";

const REPO = "RustLauncherMC/Launcher";
const RELEASES = `https://github.com/${REPO}/releases`;
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

document.documentElement.classList.add("js");

/* ---------- Navigation ---------- */
const nav = document.querySelector(".nav");
const onScroll = () => nav.classList.toggle("scrolled", window.scrollY > 40);
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
// A small deterministic random number generator, so the art is the same on every load.
function rng(seed) {
  return () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 4294967296;
  };
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

const hex = (h) => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];

/* The logo's cog in pixel art: 24 teeth around a band and an inner ring, lit from the top
   left, turning slowly behind the launcher window. Drawn pixel by pixel at a fifth of the
   canvas's size; CSS scales it up without smoothing. */
function cog(canvas) {
  const N = 152;
  canvas.width = N;
  canvas.height = N;
  const ctx = canvas.getContext("2d");
  const image = ctx.createImageData(N, N);
  const c = N / 2;
  const shades = ["#ffb39f", "#f0795a", "#d65f3a", "#b7410e", "#7c2c0c"].map(hex);
  const TEETH = 24;
  const light = (-3 * Math.PI) / 4; // from the top left

  const draw = (time) => {
    const turn = reducedMotion ? 0 : (time / 90000) * Math.PI * 2;
    const data = image.data;
    data.fill(0);
    for (let y = 0; y < N; y++) {
      for (let x = 0; x < N; x++) {
        const dx = x + 0.5 - c;
        const dy = y + 0.5 - c;
        const r = Math.hypot(dx, dy);
        const a = Math.atan2(dy, dx);
        // Which part of the cog this pixel is, and how close to its edge.
        let edge = -1;
        if (r >= 46 && r < 51) edge = Math.min(r - 46, 51 - r); // inner ring
        else if (r >= 55 && r < 63) edge = Math.min(r - 55, 63 - r); // band
        else if (r >= 63 && r < 73) {
          const t = (((a - turn) / (Math.PI * 2)) * TEETH) % 1;
          const tooth = (t + 1) % 1;
          if (tooth < 0.5) edge = Math.min(73 - r, tooth * 9, (0.5 - tooth) * 9);
        }
        if (edge < 0) continue;
        // Light: brighter facing the top left, a dark rim on every edge.
        const facing = Math.cos(a - light);
        let shade = facing > 0.55 ? 0 : facing > 0 ? 1 : facing > -0.55 ? 2 : 3;
        if (edge < 1) shade = Math.min(4, shade + 2);
        const [R, G, B] = shades[shade];
        const i = (y * N + x) * 4;
        data[i] = R;
        data[i + 1] = G;
        data[i + 2] = B;
        data[i + 3] = 255;
      }
    }
    ctx.putImageData(image, 0, 0);
  };
  animate(canvas, draw);
}

/* The community section: pixel embers rising from the bottom, like sparks off the cog. */
function embers(canvas) {
  const PIXEL = 4;
  const ctx = canvas.getContext("2d");
  const r = rng(11);
  let sparks = [];
  const colors = ["#ffb39f", "#f0795a", "#e2603c", "#b7410e", "#6db33f"];
  const draw = (time) => {
    const w = Math.ceil(canvas.clientWidth / PIXEL);
    const h = Math.ceil(canvas.clientHeight / PIXEL);
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
      sparks = Array.from({ length: Math.round(w / 3) }, () => ({
        x: r() * w,
        y: r() * h,
        speed: 0.006 + r() * 0.02,
        sway: r() * Math.PI * 2,
        color: colors[Math.floor(r() * (r() < 0.12 ? 5 : 4))],
        size: r() < 0.2 ? 2 : 1,
      }));
    }
    ctx.clearRect(0, 0, w, h);
    // A warm glow low in the section, fading out before its edges.
    const glow = ctx.createLinearGradient(0, 0, 0, h);
    glow.addColorStop(0.35, "rgba(240, 121, 90, 0)");
    glow.addColorStop(0.75, "rgba(240, 121, 90, 0.1)");
    glow.addColorStop(1, "rgba(240, 121, 90, 0)");
    ctx.fillStyle = glow;
    ctx.fillRect(0, 0, w, h);
    const t = reducedMotion ? 0 : time;
    for (const s of sparks) {
      const y = (((s.y - t * s.speed) % h) + h) % h;
      const x = s.x + Math.sin(t / 900 + s.sway) * 3;
      const life = y / h; // fades as it rises
      ctx.globalAlpha = Math.max(0, Math.min(1, life * 1.4)) * 0.8;
      ctx.fillStyle = s.color;
      ctx.fillRect(Math.round(x), Math.round(y), s.size, s.size);
    }
    ctx.globalAlpha = 1;
  };
  animate(canvas, draw);
}

cog(document.getElementById("cog"));
embers(document.getElementById("embers"));

/* ---------- Floating blocks follow the pointer a little ---------- */
const floats = document.querySelectorAll(".float");
if (!reducedMotion && floats.length && window.matchMedia("(pointer: fine)").matches) {
  window.addEventListener("pointermove", (e) => {
    const dx = e.clientX / window.innerWidth - 0.5;
    const dy = e.clientY / window.innerHeight - 0.5;
    floats.forEach((el, i) => {
      const depth = 6 + i * 4;
      el.style.transform = `translate(${(-dx * depth).toFixed(1)}px, ${(-dy * depth).toFixed(1)}px)`;
    });
  }, { passive: true });
}

/* ---------- The section in view, highlighted in the navbar ---------- */
const spyLinks = [...document.querySelectorAll("[data-spy]")];
if ("IntersectionObserver" in window && spyLinks.length) {
  const byId = new Map(spyLinks.map((a) => [a.getAttribute("href").slice(1), a]));
  const spy = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        spyLinks.forEach((a) => a.classList.remove("active"));
        const link = byId.get(entry.target.id);
        if (link) link.classList.add("active");
      }
    },
    { rootMargin: "-45% 0px -50% 0px" }
  );
  byId.forEach((_, id) => {
    const section = document.getElementById(id);
    if (section) spy.observe(section);
  });
  // Above the first section, nothing is highlighted.
  window.addEventListener("scroll", () => {
    const first = document.getElementById(spyLinks[0].getAttribute("href").slice(1));
    if (first && window.scrollY + window.innerHeight * 0.5 < first.offsetTop) {
      spyLinks.forEach((a) => a.classList.remove("active"));
    }
  }, { passive: true });
}

/* ---------- The launcher preview ---------- */
const editions = {
  java: { title: "JAVA EDITION", sub: "Horizon", name: "Horizon", tag: "Minecraft: Java Edition rewritten in Rust", status: "Latest nightly", version: "Horizon nightly" },
  bedrock: { title: "BEDROCK EDITION", sub: "Cinnabar", name: "Cinnabar", tag: "A Bedrock Edition client written in Rust", status: "Latest Beta", version: "Cinnabar Beta" },
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
    win("version").textContent = edition.version;
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
  const chip = document.querySelector("[data-chip-text]");
  if (chip) chip.textContent = `${version} is out for Windows and Linux`;
  const chipLink = document.querySelector("[data-chip]");
  if (chipLink && release.html_url) chipLink.href = release.html_url;
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
