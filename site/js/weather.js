// Weather over the whole page: rain, snow or fireflies, drawn in the
// letters the visitor picked. Off unless asked for, and never for anyone
// whose system asks for less motion.
import { events, look, inLetters, calm } from './look.js';

const KINDS = {
  rain: { per: 7000, glyph: '|' },
  snow: { per: 9000, glyphs: ['*', '·', '.'] },
  fireflies: { per: 42000, glyph: '•' },
};

export function initWeather() {
  const canvas = document.createElement('canvas');
  canvas.className = 'page-weather';
  canvas.setAttribute('aria-hidden', 'true');
  document.body.append(canvas);
  const ctx = canvas.getContext('2d');
  const probe = document.createElement('span');
  probe.className = 'weather-probe';
  document.body.append(probe);

  let kind = 'off';
  let parts = [];
  let raf = 0;
  let last = 0;
  let w = 0;
  let h = 0;
  let dpr = 1;
  let ink = 'rgb(128 128 128)';
  let glow = 'rgb(240 162 123)';

  function colours() {
    const cs = getComputedStyle(probe);
    ink = cs.color;
    glow = cs.backgroundColor;
  }

  function size() {
    dpr = Math.min(devicePixelRatio || 1, 2);
    w = innerWidth;
    h = innerHeight;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  function spawn(fromTop) {
    const r = Math.random;
    if (kind === 'rain') {
      return { x: r() * (w + 200) - 100, y: fromTop ? -20 - r() * 40 : r() * h, v: 520 + r() * 260, a: 0.18 + r() * 0.16 };
    }
    if (kind === 'snow') {
      const g = KINDS.snow.glyphs;
      return {
        x: r() * w, y: fromTop ? -10 : r() * h, v: 26 + r() * 34, a: 0.3 + r() * 0.35,
        sway: r() * Math.PI * 2, g: g[Math.floor(r() * g.length)],
      };
    }
    return { x: r() * w, y: h * 0.35 + r() * h * 0.65, vx: r() - 0.5, vy: r() - 0.5, phase: r() * 6.28, a: 0 };
  }

  function fill() {
    const n = kind === 'off' ? 0 : Math.round((w * h) / KINDS[kind].per);
    parts = Array.from({ length: n }, () => spawn(false));
  }

  function step(t) {
    raf = requestAnimationFrame(step);
    if (t - last < 33) return;
    const dt = Math.min((t - last) / 1000, 0.1);
    last = t;
    ctx.clearRect(0, 0, w, h);
    ctx.font = '13px "JetBrains Mono", ui-monospace, monospace';
    ctx.textAlign = 'center';
    if (kind === 'rain') {
      const g = inLetters(KINDS.rain.glyph);
      ctx.fillStyle = ink;
      for (const p of parts) {
        p.y += p.v * dt;
        p.x += p.v * dt * 0.12;
        if (p.y > h + 20) Object.assign(p, spawn(true));
        ctx.globalAlpha = p.a;
        ctx.fillText(g, p.x, p.y);
      }
    } else if (kind === 'snow') {
      ctx.fillStyle = ink;
      for (const p of parts) {
        p.sway += dt * 0.9;
        p.y += p.v * dt;
        p.x += Math.sin(p.sway) * 14 * dt;
        if (p.y > h + 10) Object.assign(p, spawn(true));
        ctx.globalAlpha = p.a;
        ctx.fillText(inLetters(p.g), p.x, p.y);
      }
    } else if (kind === 'fireflies') {
      const g = inLetters(KINDS.fireflies.glyph);
      ctx.fillStyle = glow;
      ctx.shadowColor = glow;
      ctx.shadowBlur = 8;
      for (const p of parts) {
        p.phase += dt * 1.6;
        p.vx += (Math.random() - 0.5) * dt * 2;
        p.vy += (Math.random() - 0.5) * dt * 2;
        p.vx *= 0.98;
        p.vy *= 0.98;
        p.x = (p.x + p.vx * 22 * dt + w) % w;
        p.y = Math.min(h, Math.max(h * 0.2, p.y + p.vy * 22 * dt));
        ctx.globalAlpha = Math.max(0, Math.sin(p.phase)) * 0.85;
        ctx.fillText(g, p.x, p.y);
      }
      ctx.shadowBlur = 0;
    }
    ctx.globalAlpha = 1;
  }

  function set(next) {
    kind = calm() ? 'off' : next in KINDS ? next : 'off';
    cancelAnimationFrame(raf);
    canvas.hidden = kind === 'off';
    if (kind === 'off') return;
    size();
    colours();
    fill();
    last = 0;
    raf = requestAnimationFrame(step);
  }

  addEventListener('resize', () => {
    if (kind === 'off') return;
    size();
    fill();
  });
  events.addEventListener('change', (e) => {
    if (e.detail.changed.includes('weather')) set(e.detail.weather);
    else if (kind !== 'off') colours();
  });
  // The theme button repaints the page.
  new MutationObserver(() => kind !== 'off' && setTimeout(colours, 50)).observe(
    document.querySelector('meta[name="color-scheme"]'),
    { attributes: true },
  );
  set(look().weather);
}
