import init, * as wasm from "./pkg/cascade_ca.js";

const $ = (id) => document.getElementById(id);

function showError(detail) {
  $("error").classList.remove("hidden");
  if (detail) $("error-detail").textContent = String(detail);
}

function layoutOverlays() {
  const h = window.innerHeight;
  const [b1, b2] = wasm.boundaries();
  $("handle1").style.top = `${b1 * h}px`;
  $("handle2").style.top = `${b2 * h}px`;
  // one info button near the top-right of each region
  $("info0").style.top = "12px";
  $("info1").style.top = `${b1 * h + 10}px`;
  $("info2").style.top = `${b2 * h + 10}px`;
}

function setPlayIcon() {
  $("btn-play").textContent = wasm.is_paused() ? "▶" : "⏸";
}

// ---------- info modal content ----------

function el(tag, cls, ...children) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const c of children) e.append(c);
  return e;
}

// spec: 9 chars — '.' empty, '#' alive, 'o' empty+outlined, '@' alive+outlined,
// 'n' highlighted as a counted neighbour, 'x' faded/gone
function miniGrid(spec, accent) {
  const g = el("div", "mini");
  g.style.setProperty("--accent", accent);
  for (const ch of spec) {
    const c = el("div", "cell");
    if (ch === "#" || ch === "@") c.classList.add("on");
    if (ch === "o" || ch === "@") c.classList.add("focus");
    if (ch === "n") c.classList.add("count");
    if (ch === "x") c.classList.add("gone");
    g.append(c);
  }
  return g;
}

function singleCell(on, accent, focus = false) {
  const c = el("div", "cell" + (on ? " on" : " gone") + (focus ? " focus" : ""));
  c.style.setProperty("--accent", accent);
  return c;
}

function figureEl(content, caption, cls = "") {
  const f = el("figure", cls, content);
  f.append(el("figcaption", "", caption));
  return f;
}

const AMBER = "#ffb847";
const MINT = "#73ffb8";
const WARM = "#ffdb80";

// Actually run Rule 30 from a centred single cell: returns `gens` rows.
function rule30Rows(width, gens) {
  let row = new Array(width).fill(0);
  row[Math.floor(width / 2)] = 1;
  const rows = [row];
  for (let t = 1; t < gens; t++) {
    row = row.map((_, x) => {
      const l = row[(x + width - 1) % width];
      const r = row[(x + 1) % width];
      return l ^ (row[x] | r);
    });
    rows.push(row);
  }
  return rows;
}

function pixGrid(rows, accent, px = 11) {
  const g = el("div", "pixgrid");
  g.style.gridTemplateColumns = `repeat(${rows[0].length}, ${px}px)`;
  g.style.setProperty("--accent", accent);
  g.style.setProperty("--px", `${px}px`);
  for (const row of rows) {
    for (const v of row) g.append(el("div", "cell" + (v ? " on" : "")));
  }
  return g;
}

// rule_str format: survival/birth/states/neighbourhood, e.g. "4-7/6-8/10/Moore"
function prettySpec(spec) {
  const parts = spec.split(",").map((s) => s.replace("-", "–"));
  if (parts.length === 1) return parts[0];
  return parts.slice(0, -1).join(", ") + " or " + parts[parts.length - 1];
}

// ---------- diagram slots ----------
// Prose lives in web/content/*.md; these build the visuals that the markdown
// drops in at each <placeholder> line, in order. Each region returns the
// ordered slot nodes plus any {{token}} values its markdown references.

function rule30Cases() {
  const dia = el("div", "diagram");
  for (let p = 7; p >= 0; p--) {
    const kase = el("div", "r30case");
    const row = el("div", "r30row");
    for (const bit of [4, 2, 1]) {
      const c = el("div", "cell" + ((p & bit) ? " on" : ""));
      c.style.setProperty("--accent", AMBER);
      row.append(c);
    }
    kase.append(row, el("span", "arrow", "↓"), singleCell(((30 >> p) & 1) === 1, AMBER));
    dia.append(kase);
  }
  return dia;
}

function photoFigure(src, alt, captionHtml) {
  const fig = el("figure", "photo");
  const img = el("img");
  img.src = src;
  img.alt = alt;
  fig.append(img);
  if (captionHtml) {
    const cap = el("figcaption");
    cap.innerHTML = captionHtml;
    fig.append(cap);
  }
  return fig;
}

function rule30Content() {
  const rows = rule30Rows(27, 12);
  const big = rule30Rows(81, 41);
  return {
    slots: [
      el("div", "diagram", figureEl(pixGrid([rows[6]], AMBER), "one generation: a single row", "wide")),
      el("div", "diagram", figureEl(pixGrid([rows[6], rows[7]], AMBER), "new generation underneath", "wide")),
      rule30Cases(),
      el("div", "diagram", figureEl(pixGrid(big, AMBER, 5), "40 generations from a single cell", "wide")),
      // <laplace>: portrait with attribution caption
      photoFigure(
        "images/laplace.jpg",
        "Portrait of Pierre-Simon Laplace",
        'Pierre-Simon Laplace (1749-1827). Portrait via <a href="https://en.wikipedia.org/wiki/Pierre-Simon_Laplace" target="_blank" rel="noopener">Wikipedia</a>.'
      ),
      // <shell>: caption is the markdown paragraph that follows the placeholder
      photoFigure(
        "images/conus-textile.jpg",
        "A Conus textile shell, patterned strikingly like Rule 30"
      ),
    ],
  };
}

// ---------- animated Game of Life pattern demos ----------
// Each board runs its own interval; golStopBoards() tears them down when the
// modal closes or another popup opens (intervals would otherwise outlive the
// detached DOM nodes).
let golTimers = [];
function golStopBoards() {
  for (const t of golTimers) clearInterval(t);
  golTimers = [];
}

// live: array of [x, y] coordinates. Options:
//   wrap       toroidal neighbours (a lone glider then crawls forever)
//   sink       clear the 2-cell border each step so travellers exit cleanly
//   resetEvery reseed after N generations (belt-and-braces against drift)
function golBoard(w, h, live, { px = 11, ms = 160, wrap = false, sink = false, resetEvery = 0 } = {}) {
  const grid = el("div", "pixgrid");
  grid.style.gridTemplateColumns = `repeat(${w}, ${px}px)`;
  grid.style.setProperty("--accent", MINT);
  grid.style.setProperty("--px", `${px}px`);
  const divs = [];
  for (let i = 0; i < w * h; i++) {
    const c = el("div", "cell");
    divs.push(c);
    grid.append(c);
  }

  let state = new Uint8Array(w * h);
  const seed = () => {
    state.fill(0);
    for (const [x, y] of live) if (x >= 0 && x < w && y >= 0 && y < h) state[y * w + x] = 1;
  };
  const draw = () => {
    for (let i = 0; i < w * h; i++) divs[i].classList.toggle("on", state[i] === 1);
  };

  let gen = 0;
  const tick = () => {
    if (resetEvery && gen >= resetEvery) { seed(); draw(); gen = 0; return; }
    const next = new Uint8Array(w * h);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        let n = 0;
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;
            let nx = x + dx, ny = y + dy;
            if (wrap) { nx = (nx + w) % w; ny = (ny + h) % h; }
            else if (nx < 0 || nx >= w || ny < 0 || ny >= h) continue;
            n += state[ny * w + nx];
          }
        }
        const alive = state[y * w + x] === 1;
        next[y * w + x] = (alive ? n === 2 || n === 3 : n === 3) ? 1 : 0;
      }
    }
    state = next;
    if (sink) {
      for (let y = 0; y < h; y++)
        for (let x = 0; x < w; x++)
          if (x < 2 || x >= w - 2 || y < 2 || y >= h - 2) state[y * w + x] = 0;
    }
    draw();
    gen++;
  };

  seed();
  draw();
  golTimers.push(setInterval(tick, ms));
  return grid;
}

function oscillatorBoard() {
  // blinker (period 2)
  return golBoard(7, 7, [[2, 3], [3, 3], [4, 3]], { px: 14, ms: 480 });
}

function gliderBoard() {
  return golBoard(13, 13, [[1, 0], [2, 1], [0, 2], [1, 2], [2, 2]], { px: 12, ms: 150, wrap: true });
}

function gunBoard() {
  // Gosper glider gun (period 30), shifted clear of the sink border.
  const base = [
    [0, 4], [0, 5], [1, 4], [1, 5],
    [10, 4], [10, 5], [10, 6],
    [11, 3], [11, 7],
    [12, 2], [12, 8],
    [13, 2], [13, 8],
    [14, 5],
    [15, 3], [15, 7],
    [16, 4], [16, 5], [16, 6],
    [17, 5],
    [20, 2], [20, 3], [20, 4],
    [21, 2], [21, 3], [21, 4],
    [22, 1], [22, 5],
    [24, 0], [24, 1], [24, 5], [24, 6],
    [34, 2], [34, 3], [35, 2], [35, 3],
  ];
  const cells = base.map(([x, y]) => [x + 3, y + 3]);
  return golBoard(52, 36, cells, { px: 7, ms: 90, sink: true, resetEvery: 320 });
}

// Schematic of an AND gate fed by two glider "signal" streams.
function logicGateDiagram() {
  const box = el("div", "gate");
  box.innerHTML = `
    <svg viewBox="0 0 260 120" role="img" aria-label="An AND logic gate fed by two glider signals">
      <line x1="20" y1="42" x2="92" y2="42" class="wire"/>
      <line x1="20" y1="78" x2="92" y2="78" class="wire"/>
      <line x1="164" y1="60" x2="242" y2="60" class="wire"/>
      <path d="M92,28 L132,28 A32,32 0 0 1 132,92 L92,92 Z" class="gate-body"/>
      <text x="106" y="65" class="gate-label">AND</text>
      <g class="glider">
        <rect x="44" y="34" width="3.4" height="3.4"/><rect x="48" y="38" width="3.4" height="3.4"/>
        <rect x="40" y="42" width="3.4" height="3.4"/><rect x="44" y="42" width="3.4" height="3.4"/><rect x="48" y="42" width="3.4" height="3.4"/>
        <rect x="60" y="70" width="3.4" height="3.4"/><rect x="64" y="74" width="3.4" height="3.4"/>
        <rect x="56" y="78" width="3.4" height="3.4"/><rect x="60" y="78" width="3.4" height="3.4"/><rect x="64" y="78" width="3.4" height="3.4"/>
      </g>
      <text x="14" y="46" class="io" text-anchor="end">A</text>
      <text x="14" y="82" class="io" text-anchor="end">B</text>
      <text x="246" y="64" class="io">out</text>
    </svg>`;
  return el("div", "diagram",
    figureEl(box, "gliders are signals; carefully timed collisions form logic gates like this AND", "wide"));
}

function golContent() {
  const dia = el("div", "diagram");
  const cases = [
    ["..#.o.#.#", true, "Born: a dead cell with exactly 3 live neighbours"],
    ["#...@...#", true, "Survives: 2 or 3 live neighbours"],
    [".#..@....", false, "Dies of loneliness: fewer than 2"],
    ["#.#.@.#.#", false, "Dies of overcrowding: more than 3"],
  ];
  for (const [spec, outcome, caption] of cases) {
    const flow = el("div", "flow",
      miniGrid(spec, MINT),
      el("span", "arrow", "→"),
      singleCell(outcome, MINT, true));
    dia.append(figureEl(flow, caption));
  }
  return {
    slots: [
      dia, // <rules>
      // <conway>
      photoFigure(
        "images/conway.webp",
        "Portrait of John Conway",
        'Mathematician <a href="https://en.wikipedia.org/wiki/John_Horton_Conway" target="_blank" rel="noopener">John Conway</a> (1937–2020), who devised the Game of Life.'
      ),
      el("div", "diagram", figureEl(oscillatorBoard(), "a blinker ")), // <oscillator>
      el("div", "diagram", figureEl(gliderBoard(), "a glider")), // <glider>
      el("div", "diagram", figureEl(gunBoard(), "a Gosper glider gun (fires a glider every 30 generations)", "wide")), // <gun>
      // <alan-turing>
      photoFigure(
        "images/alan-turing.jpg",
        "Portrait of Alan Turing",
        'Mathematician <a href="https://en.wikipedia.org/wiki/Alan_Turing" target="_blank" rel="noopener">Alan Turing</a> (1912–1954).'
      ),
      // <turing-machine>
      photoFigure("images/turing-machine.png", "Diagram of a Turing machine"),
      logicGateDiagram(), // <gol-logic-gate>
    ],
  };
}

function ca3dContent() {
  const i = wasm.current_preset();
  const [survival, birth, states, nbhd] = wasm.preset_rule_str(i).split("/");
  const moore = nbhd.toLowerCase().includes("moore");
  const nStates = Number(states);

  const chips = el("div", "chips");
  const chip = (html) => { const c = el("span", "chip"); c.innerHTML = html; return c; };
  chips.append(
    chip(`born with <b>${prettySpec(birth)}</b> live neighbours`),
    chip(`survives with <b>${prettySpec(survival)}</b>`),
  );
  if (nStates > 2) chips.append(chip(`fades out over <b>${nStates - 2}</b> ticks after dying`));
  chips.append(chip(moore ? `counts all <b>26</b> touching cells` : `counts only the <b>6</b> face-to-face cells`));

  const layers = el("div", "diagram");
  const mid = moore ? "nnnnonnnn" : ".n.non.n.";
  const outer = moore ? "nnnnnnnnn" : "....n....";
  for (const [spec, caption] of [
    [outer, "layer below"],
    [mid, "its own layer"],
    [outer, "layer above"],
  ]) {
    layers.append(figureEl(miniGrid(spec, WARM), caption));
  }

  const slots = [chips, layers];
  if (nStates > 2) {
    const strip = el("div", "fade-strip");
    strip.append(singleCell(true, WARM, false), el("span", "lbl", "alive"));
    for (let s = 0; s < Math.min(nStates - 2, 8); s++) {
      const t = s / Math.max(nStates - 3, 1);
      const c = el("div", "cell");
      const lerp = (a, b) => Math.round(a + (b - a) * t);
      c.style.background = `rgb(${lerp(238, 77)}, ${lerp(89, 31)}, ${lerp(76, 115)})`;
      strip.append(c);
    }
    strip.append(el("span", "lbl", "fading"), singleCell(false, WARM), el("span", "lbl", "gone"));
    slots.push(strip);
  }

  return {
    slots,
    vars: { name: wasm.preset_name(i), blurb: wasm.preset_blurb(i), moore, fades: nStates > 2 },
  };
}

// ---------- tiny markdown renderer ----------
// Supports: headings (first heading = modal title, rest = <h3> sections),
// paragraphs, **bold**, [links](url), <placeholder> slot lines, and
// {{token}} / {{#section}}…{{/section}} / {{^section}}…{{/section}} from vars.

function renderInline(text) {
  const h = text
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>');
  return h;
}

// A line that is just a YouTube URL becomes a responsive embedded player.
const YOUTUBE_RE = /^https?:\/\/(?:www\.)?(?:youtube\.com\/watch\?v=|youtu\.be\/)([\w-]{6,})/;
function youtubeEmbed(id) {
  const wrap = el("div", "video");
  const iframe = document.createElement("iframe");
  iframe.src = `https://www.youtube-nocookie.com/embed/${id}`;
  iframe.title = "YouTube video player";
  iframe.allow = "accelerometer; clipboard-write; encrypted-media; gyroscope; picture-in-picture";
  iframe.allowFullscreen = true;
  iframe.loading = "lazy";
  wrap.append(iframe);
  return wrap;
}

function renderMarkdown(md, { slots = [], vars = {} } = {}) {
  md = md
    .replace(/\{\{#(\w+)\}\}([\s\S]*?)\{\{\/\1\}\}/g, (_, k, inner) => (vars[k] ? inner : ""))
    .replace(/\{\{\^(\w+)\}\}([\s\S]*?)\{\{\/\1\}\}/g, (_, k, inner) => (vars[k] ? "" : inner))
    .replace(/\{\{(\w+)\}\}/g, (_, k) => (k in vars ? String(vars[k]) : ""));

  const bodyEl = el("div");
  let title = null;
  let slotIdx = 0;
  let para = [];
  const flush = () => {
    if (para.length) {
      const p = el("p");
      p.innerHTML = renderInline(para.join(" "));
      bodyEl.append(p);
      para = [];
    }
  };
  for (const raw of md.split("\n")) {
    const line = raw.trim();
    if (!line) { flush(); continue; }
    if (/^#{1,6}\s/.test(line)) {
      flush();
      const text = line.replace(/^#{1,6}\s*/, "");
      if (title === null) title = text;
      else bodyEl.append(el("h3", "", text));
    } else if (/^<[^>]+>$/.test(line)) {
      flush();
      const node = slots[slotIdx++];
      if (node) bodyEl.append(node);
    } else if (YOUTUBE_RE.test(line)) {
      flush();
      bodyEl.append(youtubeEmbed(line.match(YOUTUBE_RE)[1]));
    } else {
      para.push(line);
    }
  }
  flush();
  return { title, body: bodyEl };
}

// ---------- info modal wiring ----------

const CONTENT_FILES = { 0: "ca3d", 1: "gol", 2: "rule30" };
const CONTENT_BUILDERS = { 0: ca3dContent, 1: golContent, 2: rule30Content };
const contentCache = {};

async function loadContent() {
  await Promise.all(Object.entries(CONTENT_FILES).map(async ([region, name]) => {
    try {
      // cache-bust so edits to the .md files show on a normal reload
      const res = await fetch(`content/${name}.md?t=${Date.now()}`, { cache: "no-store" });
      contentCache[region] = res.ok ? await res.text() : null;
    } catch {
      contentCache[region] = null;
    }
  }));
}

function openInfo(region) {
  golStopBoards(); // stop any Life animations from a previously opened popup
  const md = contentCache[region];
  if (md == null) {
    $("modal-title").textContent = "Info unavailable";
    $("modal-body").replaceChildren(
      el("p", "", "Could not load the explanation text. Make sure the page is served over HTTP (not opened as a file)."));
  } else {
    const { title, body } = renderMarkdown(md, CONTENT_BUILDERS[region]());
    $("modal-title").textContent = title || "";
    $("modal-body").replaceChildren(body);
  }
  $("modal-backdrop").classList.remove("hidden");
}

function closeInfo() {
  golStopBoards();
  $("modal-backdrop").classList.add("hidden");
}

function setupDrag(handleEl, which) {
  handleEl.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    handleEl.setPointerCapture(e.pointerId);
    handleEl.classList.add("dragging");
    const move = (ev) => {
      const h = window.innerHeight;
      let [b1, b2] = wasm.boundaries();
      const frac = ev.clientY / h;
      if (which === 1) b1 = frac; else b2 = frac;
      wasm.set_boundaries(b1, b2);
      layoutOverlays();
    };
    const up = (ev) => {
      handleEl.classList.remove("dragging");
      if (handleEl.hasPointerCapture?.(e.pointerId)) handleEl.releasePointerCapture(e.pointerId);
      handleEl.removeEventListener("pointermove", move);
      handleEl.removeEventListener("pointerup", up);
      handleEl.removeEventListener("pointercancel", up);
    };
    handleEl.addEventListener("pointermove", move);
    handleEl.addEventListener("pointerup", up);
    handleEl.addEventListener("pointercancel", up);
  });
}

// 3D camera panel. Azimuth/elevation/distance are driven by drag-to-orbit and
// wheel/pinch zoom on the canvas, so they appear as live read-outs only; only
// target height and field of view (awkward to set by gesture) keep sliders.
function setupCamera() {
  // camera_values() order: [az, el, dist, ty, fov]
  const out = Object.fromEntries(["az", "el", "dist", "ty", "fov"].map((k) => [k, $(`cam-${k}-v`)]));
  const inp = { ty: $("cam-ty"), fov: $("cam-fov") };
  const fmt = (x) => `${Math.round(x * 10) / 10}`; // tidy 1-dp display, drops trailing .0

  // Push a full [az, el, dist, ty, fov] to the renderer and reflect it in the
  // read-outs and the two slider positions.
  const setAll = (v) => {
    wasm.set_camera(...v);
    out.az.textContent = fmt(v[0]); out.el.textContent = fmt(v[1]); out.dist.textContent = fmt(v[2]);
    out.ty.textContent = fmt(v[3]); out.fov.textContent = fmt(v[4]);
    inp.ty.value = v[3]; inp.fov.value = v[4];
  };

  // Sliders only change ty/fov; az/el/dist stay as whatever the gestures set.
  const onSlider = () => {
    const v = wasm.camera_values();
    v[3] = Number(inp.ty.value);
    v[4] = Number(inp.fov.value);
    setAll(v);
  };
  inp.ty.addEventListener("input", onSlider);
  inp.fov.addEventListener("input", onSlider);
  $("cam-head").addEventListener("click", () => $("cam-panel").classList.toggle("collapsed"));
  $("cam-reset").addEventListener("click", () => setAll(wasm.camera_defaults()));

  setAll(wasm.camera_values());

  // Expose setAll so the canvas gestures drive the same camera and keep the
  // read-outs (and ty/fov sliders) in sync.
  return { setAll };
}

// Drag on the 3D (top) region to orbit; wheel/trackpad and two-finger pinch to
// zoom. Reads the live camera via wasm.camera_values() and writes back through
// cam.setAll, which refreshes the az/el/dist read-outs.
function setupOrbit(cam) {
  const canvas = $("ca-canvas");
  const { setAll } = cam;
  const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));
  const wrap180 = (d) => ((d + 180) % 360 + 360) % 360 - 180;
  const elMin = -15, elMax = 89;     // elevation clamp (degrees)
  const distMin = 30, distMax = 320; // distance clamp
  const ORBIT_SENS = 0.4;    // degrees rotated per CSS pixel dragged
  const WHEEL_SENS = 0.0015; // zoom factor exponent per wheel delta unit

  // The 3D region is the top slice of the canvas, above the first boundary.
  const inTop = (clientY) => clientY < wasm.boundaries()[0] * window.innerHeight;

  const pointers = new Map(); // active pointerId -> {x, y}, only those started in the top region
  let pinchDist = 0;          // last two-finger spacing; 0 when not pinching
  const spacing = () => {
    const [a, b] = [...pointers.values()];
    return Math.hypot(a.x - b.x, a.y - b.y);
  };

  canvas.addEventListener("pointerdown", (e) => {
    if (!inTop(e.clientY)) return;
    canvas.setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 2) pinchDist = spacing();
    canvas.classList.add("orbiting");
  });

  canvas.addEventListener("pointermove", (e) => {
    const p = pointers.get(e.pointerId);
    if (!p) return;
    const dx = e.clientX - p.x, dy = e.clientY - p.y;
    p.x = e.clientX; p.y = e.clientY;
    const v = wasm.camera_values();
    if (pointers.size >= 2) {
      // pinch to zoom: fingers apart -> camera moves closer
      const s = spacing();
      if (pinchDist > 0 && s > 0) {
        v[2] = clamp(v[2] * (pinchDist / s), distMin, distMax);
        setAll(v);
      }
      pinchDist = s;
    } else {
      // single pointer drag to orbit (drag up tilts the view up)
      v[0] = wrap180(v[0] + dx * ORBIT_SENS);
      v[1] = clamp(v[1] - dy * ORBIT_SENS, elMin, elMax);
      setAll(v);
    }
  });

  const release = (e) => {
    if (!pointers.delete(e.pointerId)) return;
    if (canvas.hasPointerCapture?.(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
    if (pointers.size < 2) pinchDist = 0;
    if (pointers.size === 0) canvas.classList.remove("orbiting");
  };
  canvas.addEventListener("pointerup", release);
  canvas.addEventListener("pointercancel", release);

  canvas.addEventListener("wheel", (e) => {
    if (!inTop(e.clientY)) return;
    e.preventDefault();
    const v = wasm.camera_values();
    v[2] = clamp(v[2] * Math.exp(e.deltaY * WHEEL_SENS), distMin, distMax);
    setAll(v);
  }, { passive: false });
}

// Telltale substrings for CPU/software WebGPU backends (SwiftShader, WARP,
// Mesa llvmpipe/lavapipe, etc.).
const SOFTWARE_RE = /swiftshader|llvmpipe|lavapipe|software|microsoft basic|basic render|\bwarp\b/i;

// Probe the adapter the renderer will get, so we can warn before booting.
// Mirrors the options used by the Rust side (high-performance, no forced fallback).
// Returns { ok, software, info, detail }.
async function inspectAdapter() {
  if (!navigator.gpu) return { ok: false };
  let adapter;
  try {
    adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  } catch (e) {
    return { ok: false, detail: e };
  }
  if (!adapter) return { ok: false };

  // GPUAdapterInfo is a sync property in recent browsers; older ones expose
  // an async requestAdapterInfo().
  let info = adapter.info;
  if (!info && typeof adapter.requestAdapterInfo === "function") {
    try { info = await adapter.requestAdapterInfo(); } catch { /* ignore */ }
  }
  const blob = info
    ? [info.vendor, info.architecture, info.device, info.description].filter(Boolean).join(" ").trim()
    : "";

  const software = adapter.isFallbackAdapter === true || SOFTWARE_RE.test(blob);
  return { ok: true, software, info: blob };
}

async function main() {
  const status = await inspectAdapter();
  if (!status.ok) {
    showError(status.detail);
    return;
  }
  if (status.software) {
    if (status.info) $("warn-info").textContent = `Renderer: ${status.info}`;
    $("warn-software").classList.remove("hidden");
    $("warn-proceed").addEventListener("click", () => {
      $("warn-software").classList.add("hidden");
      boot();
    }, { once: true });
    return;
  }
  await boot();
}

// Initialise the wasm module and wire up the full UI. Called either directly
// (hardware GPU) or after the user clicks "Proceed anyway" on the software warning.
async function boot() {
  await init();
  try {
    await wasm.start("ca-canvas", window.innerWidth, window.innerHeight, window.devicePixelRatio || 1);
  } catch (e) {
    console.error(e);
    showError(e);
    return;
  }

  // presets
  const sel = $("preset");
  for (let i = 0; i < wasm.preset_count(); i++) {
    const opt = document.createElement("option");
    opt.value = i;
    opt.textContent = wasm.preset_name(i);
    sel.appendChild(opt);
  }
  sel.value = wasm.current_preset();
  sel.addEventListener("change", () => {
    wasm.set_preset(Number(sel.value));
    sel.blur(); // keep Space for pause, not the select
  });

  // transport controls
  $("btn-play").addEventListener("click", () => { wasm.toggle_pause(); setPlayIcon(); });
  $("btn-fwd").addEventListener("click", () => { wasm.step_forward(); setPlayIcon(); });
  $("btn-back").addEventListener("click", () => { wasm.step_backward(); setPlayIcon(); });
  $("btn-reset").addEventListener("click", () => wasm.reset());
  $("speed").addEventListener("input", (e) => wasm.set_speed(Number(e.target.value)));

  // keyboard
  window.addEventListener("keydown", (e) => {
    if (e.target.tagName === "INPUT" || e.target.tagName === "SELECT") return;
    switch (e.code) {
      case "Space":
        e.preventDefault();
        wasm.toggle_pause();
        setPlayIcon();
        break;
      case "ArrowRight":
      case "Period":
        e.preventDefault();
        wasm.step_forward();
        setPlayIcon();
        break;
      case "ArrowLeft":
      case "Comma":
        e.preventDefault();
        wasm.step_backward();
        setPlayIcon();
        break;
      case "Escape":
        closeInfo();
        break;
    }
  });

  // info popups (prose loaded from web/content/*.md)
  await loadContent();
  for (const region of [0, 1, 2]) {
    $(`info${region}`).addEventListener("click", () => openInfo(region));
  }
  $("modal-close").addEventListener("click", closeInfo);
  $("modal-backdrop").addEventListener("click", (e) => {
    if (e.target.id === "modal-backdrop") closeInfo();
  });

  // draggable region boundaries
  setupDrag($("handle1"), 1);
  setupDrag($("handle2"), 2);

  // 3D camera tuning panel, plus drag-to-orbit / pinch-&-wheel-zoom on the canvas
  const cam = setupCamera();
  setupOrbit(cam);

  // window resizes
  let resizeTimer = null;
  window.addEventListener("resize", () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      wasm.resize(window.innerWidth, window.innerHeight, window.devicePixelRatio || 1);
      layoutOverlays();
    }, 120);
  });

  layoutOverlays();
  setPlayIcon();

  // deep link: ?info=0|1|2 opens that region's explainer
  const info = new URLSearchParams(location.search).get("info");
  if (info !== null) openInfo(Math.min(2, Math.max(0, Number(info) || 0)));
}

main();
