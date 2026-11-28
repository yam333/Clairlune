// The page's side of the engine: views (a canvas showing a scene), PNGs and
// Clawd's frames. The work happens in engine-worker.js where the browser can
// hand a canvas to a worker, and on the page otherwise.

const WASM = new URL('../engine/clairlune.wasm', import.meta.url).href;

let post = null;
let seq = 0;
const waiting = new Map();
const viewsById = new Map();
let nextId = 1;

function receive(m) {
  if (m.type === 'info' || m.type === 'error') {
    const v = viewsById.get(m.id);
    if (v && m.type === 'error' && m.seq === undefined) v.onerror?.(m.message);
  }
  const w = m.seq !== undefined && waiting.get(m.seq);
  if (!w) return;
  waiting.delete(m.seq);
  if (m.type === 'error') w.reject(new Error(m.message));
  else w.resolve(m);
}

function ask(msg, transfer) {
  const s = ++seq;
  return new Promise((resolve, reject) => {
    waiting.set(s, { resolve, reject });
    post({ ...msg, seq: s }, transfer);
  });
}

const offscreen =
  typeof Worker !== 'undefined' &&
  typeof HTMLCanvasElement !== 'undefined' &&
  'transferControlToOffscreen' in HTMLCanvasElement.prototype;

export const ready = (async () => {
  if (offscreen) {
    const worker = new Worker(new URL('./engine-worker.js', import.meta.url), { type: 'module' });
    await new Promise((resolve, reject) => {
      worker.onmessage = (e) => {
        if (e.data.type === 'ready') resolve();
        else if (e.data.type === 'failed') reject(new Error(e.data.message));
        else receive(e.data);
      };
      worker.onerror = (e) => reject(new Error(e.message || 'engine worker failed'));
      worker.postMessage({ type: 'init', url: WASM });
    });
    worker.onmessage = (e) => receive(e.data);
    post = (m, t) => worker.postMessage(m, t || []);
  } else {
    const { Core, createHost } = await import('./engine-core.js');
    const host = createHost(await Core.load(WASM), (m) => receive(m));
    post = (m) => host(m);
  }
  document.addEventListener('visibilitychange', () =>
    post({ type: 'hidden', hidden: document.visibilityState === 'hidden' }),
  );
  return true;
})();

const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)');
export const prefersStill = () => reduceMotion.matches;

const dpr = () => Math.min(window.devicePixelRatio || 1, 2.5);

// A canvas showing a scene, live or still. Its pixels are the scene's own
// (as many as the canvas covers on screen, times the display's density).
export class View {
  constructor(canvas, { fps = 12, aspect = 16 / 9, maxWidth = 1920, still = false } = {}) {
    this.canvas = canvas;
    this.id = nextId++;
    this.fps = fps;
    this.aspect = aspect;
    this.maxWidth = maxWidth;
    this.still = still;
    this.playing = false;
    this.opts = null;
    this.size = this.measure();
    viewsById.set(this.id, this);
    this.sent = false;
    this.io = new IntersectionObserver((entries) => {
      const visible = entries.some((e) => e.isIntersecting);
      if (this.sent) post({ type: 'visible', id: this.id, visible });
    });
    this.io.observe(canvas);
    if (!still) {
      let t = 0;
      this.ro = new ResizeObserver(() => {
        clearTimeout(t);
        t = setTimeout(() => {
          const s = this.measure();
          if (!this.sent || Math.abs(s.width - this.size.width) < this.size.width * 0.08) return;
          this.size = s;
          post({ type: 'resize', id: this.id, ...s });
        }, 200);
      });
      this.ro.observe(canvas);
    }
  }

  // The engine draws nothing smaller than 160x90.
  measure() {
    const css = this.canvas.clientWidth || this.canvas.parentElement?.clientWidth || 640;
    let width = Math.round(Math.min(css * dpr(), this.maxWidth));
    width = Math.max(160, width - (width % 2), Math.ceil(90 * this.aspect));
    return { width, height: Math.max(90, Math.round(width / this.aspect)) };
  }

  // Shows a scene; resolves with what it is.
  async set(opts, { play = this.playing, restart = true } = {}) {
    await ready;
    this.opts = opts;
    this.playing = play && !this.still;
    const msg = {
      type: 'view',
      id: this.id,
      opts,
      ...this.size,
      fps: this.fps,
      still: this.still,
      play: this.playing,
      restart,
    };
    let transfer = [];
    if (!this.sent) {
      msg.canvas = offscreen ? this.canvas.transferControlToOffscreen() : this.canvas;
      if (offscreen) transfer = [msg.canvas];
      this.sent = true;
    }
    const m = await ask(msg, transfer);
    return m.info;
  }

  play() {
    if (this.still || !this.sent) return;
    this.playing = true;
    post({ type: 'play', id: this.id });
  }

  pause() {
    this.playing = false;
    if (this.sent) post({ type: 'pause', id: this.id });
  }

  free() {
    this.io.disconnect();
    this.ro?.disconnect();
    viewsById.delete(this.id);
    if (this.sent) post({ type: 'free', id: this.id });
  }
}

// A PNG of a scene at any size (a wallpaper for this screen, say).
export async function png(opts) {
  await ready;
  const m = await ask({ type: 'png', opts });
  if (!m.bytes) throw new Error('could not encode the PNG');
  return { blob: new Blob([m.bytes], { type: 'image/png' }), info: m.info };
}

const clawdCache = new Map();
export async function clawd(state) {
  if (!clawdCache.has(state)) {
    clawdCache.set(
      state,
      ready.then(() => ask({ type: 'clawd', state })).then((m) => m.frames),
    );
  }
  return clawdCache.get(state);
}

export function save(blob, name) {
  const url = URL.createObjectURL(blob);
  const a = Object.assign(document.createElement('a'), { href: url, download: name });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 4000);
}

// The same seed for everyone on a given day (FNV-1a of the date).
export function seedOfDay(date = new Date()) {
  const key = `clairlune ${date.getFullYear()}-${date.getMonth() + 1}-${date.getDate()}`;
  let h = 0xcbf29ce484222325n;
  for (const c of key) {
    h ^= BigInt(c.charCodeAt(0));
    h = (h * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return (h % 1000000n).toString();
}

// A short random seed (any whole number works; six digits are easy to share).
export function randomSeed() {
  const a = new Uint32Array(1);
  crypto.getRandomValues(a);
  return String(1 + (a[0] % 999999));
}
