// The engine's host: the WebAssembly module (crates/web) plus the canvases it
// draws on. It runs in a worker that owns the canvases (OffscreenCanvas), or
// on the page itself where a browser can't hand canvases to a worker.
// Messages in, messages out; the page never touches the module directly.

const enc = new TextEncoder();
const dec = new TextDecoder();

export class Core {
  static async load(url) {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`engine: ${res.status}`);
    const { instance } = WebAssembly.instantiateStreaming
      ? await WebAssembly.instantiateStreaming(res, {}).catch(async () =>
          WebAssembly.instantiate(await (await fetch(url)).arrayBuffer(), {}))
      : await WebAssembly.instantiate(await res.arrayBuffer(), {});
    return new Core(instance.exports);
  }

  constructor(w) {
    this.w = w;
  }

  put(data) {
    const bytes = typeof data === 'string' ? enc.encode(data) : data;
    const at = this.w.cl_input(bytes.length);
    new Uint8Array(this.w.memory.buffer, at, bytes.length).set(bytes);
  }

  out() {
    return new Uint8Array(this.w.memory.buffer, this.w.cl_output(), this.w.cl_output_len());
  }

  catalog() {
    this.w.cl_catalog();
    return JSON.parse(dec.decode(this.out()));
  }

  scene(opts) {
    this.put(JSON.stringify(opts));
    const handle = this.w.cl_scene_new();
    const text = dec.decode(this.out());
    if (!handle) throw new Error(text);
    return { handle, info: JSON.parse(text) };
  }

  // Moves a scene to `t` and copies what changed onto `ctx`.
  frame(handle, t, ctx, width, height) {
    const n = this.w.cl_scene_frame(handle, t);
    if (!n) return 0;
    const buf = this.w.memory.buffer;
    const img = new ImageData(
      new Uint8ClampedArray(buf, this.w.cl_scene_rgba(handle), width * height * 4),
      width,
      height,
    );
    const r = new Uint32Array(buf, this.w.cl_scene_rects(handle), n * 4);
    for (let i = 0; i < n; i++) {
      ctx.putImageData(img, 0, 0, r[i * 4], r[i * 4 + 1], r[i * 4 + 2], r[i * 4 + 3]);
    }
    return n;
  }

  heroMs(handle) {
    return this.w.cl_scene_hero_ms(handle);
  }

  png(handle) {
    const n = this.w.cl_scene_png(handle);
    return n ? this.out().slice().buffer : null;
  }

  free(handle) {
    this.w.cl_scene_free(handle);
  }

  // Clawd's frames for one of his states: [{ w, h, ms, ox, oy, rgba }].
  clawd(state) {
    this.put(state);
    const n = this.w.cl_clawd();
    if (!n) return [];
    const bytes = this.out().slice();
    const dv = new DataView(bytes.buffer);
    const count = dv.getUint32(0, true);
    const frames = [];
    let px = 4 + count * 20;
    for (let i = 0; i < count; i++) {
      const at = 4 + i * 20;
      const f = {
        w: dv.getUint32(at, true),
        h: dv.getUint32(at + 4, true),
        ms: dv.getUint32(at + 8, true),
        ox: dv.getUint32(at + 12, true),
        oy: dv.getUint32(at + 16, true),
      };
      f.rgba = bytes.slice(px, px + f.w * f.h * 4);
      px += f.w * f.h * 4;
      frames.push(f);
    }
    return frames;
  }
}

// Runs the views: their scenes, their clocks, and drawing.
export function createHost(core, post) {
  const views = new Map();
  let hidden = false;
  let timer = 0;

  const now = () => performance.now();

  function schedule() {
    clearTimeout(timer);
    let next = Infinity;
    for (const v of views.values()) {
      if (v.handle && v.playing && v.visible && !hidden) next = Math.min(next, v.due);
    }
    if (next < Infinity) timer = setTimeout(tick, Math.max(0, next - now()));
  }

  function tick() {
    const t = now();
    for (const v of views.values()) {
      if (!(v.handle && v.playing && v.visible && !hidden) || t < v.due) continue;
      v.elapsed += Math.min(t - v.last, 250);
      v.last = t;
      v.due = t + 1000 / v.fps;
      core.frame(v.handle, v.hero + v.elapsed, v.ctx, v.width, v.height);
    }
    schedule();
  }

  function make(v) {
    if (v.handle) core.free(v.handle);
    v.handle = 0;
    const { handle, info } = core.scene({ ...v.opts, width: v.width, height: v.height });
    v.handle = handle;
    v.hero = core.heroMs(handle);
    if (v.canvas.width !== v.width) v.canvas.width = v.width;
    if (v.canvas.height !== v.height) v.canvas.height = v.height;
    v.ctx = v.canvas.getContext('2d', { alpha: false });
    core.frame(handle, v.hero + v.elapsed, v.ctx, v.width, v.height);
    v.last = now();
    v.due = v.last + 1000 / v.fps;
    if (v.still) {
      core.free(handle);
      v.handle = 0;
    }
    return info;
  }

  const on = {
    view(m) {
      let v = views.get(m.id);
      if (!v) {
        v = { canvas: m.canvas, handle: 0, elapsed: 0, playing: false, visible: true };
        views.set(m.id, v);
      }
      Object.assign(v, {
        opts: m.opts,
        width: m.width,
        height: m.height,
        fps: m.fps || 12,
        still: !!m.still,
        playing: !!m.play && !m.still,
      });
      if (m.restart) v.elapsed = 0;
      try {
        post({ type: 'info', id: m.id, seq: m.seq, info: make(v) });
      } catch (e) {
        post({ type: 'error', id: m.id, seq: m.seq, message: String(e.message || e) });
      }
      schedule();
    },
    resize(m) {
      const v = views.get(m.id);
      if (!v || (v.width === m.width && v.height === m.height)) return;
      v.width = m.width;
      v.height = m.height;
      if (!v.opts) return;
      try {
        make(v);
      } catch (e) {
        post({ type: 'error', id: m.id, message: String(e.message || e) });
      }
      schedule();
    },
    play(m) {
      const v = views.get(m.id);
      if (!v || v.still) return;
      if (!v.playing) v.last = now();
      v.playing = true;
      v.due = now();
      schedule();
    },
    pause(m) {
      const v = views.get(m.id);
      if (v) v.playing = false;
      schedule();
    },
    visible(m) {
      const v = views.get(m.id);
      if (!v) return;
      if (m.visible && !v.visible) v.last = now();
      v.visible = m.visible;
      schedule();
    },
    hidden(m) {
      if (hidden && !m.hidden) {
        const t = now();
        for (const v of views.values()) v.last = t;
      }
      hidden = m.hidden;
      schedule();
    },
    free(m) {
      const v = views.get(m.id);
      if (v?.handle) core.free(v.handle);
      views.delete(m.id);
      schedule();
    },
    png(m) {
      let handle = 0;
      try {
        const made = core.scene(m.opts);
        handle = made.handle;
        const bytes = core.png(handle);
        post({ type: 'png', seq: m.seq, bytes, info: made.info }, bytes ? [bytes] : []);
      } catch (e) {
        post({ type: 'error', seq: m.seq, message: String(e.message || e) });
      } finally {
        if (handle) core.free(handle);
      }
    },
    clawd(m) {
      const frames = core.clawd(m.state);
      post({ type: 'clawd', seq: m.seq, frames }, frames.map((f) => f.rgba.buffer));
    },
  };

  return (m) => on[m.type]?.(m);
}
