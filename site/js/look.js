// The look the visitor picked: a palette (which repaints the whole site),
// a letter set (which redraws the page's text art) and page weather. Kept in
// this browser only, and shared by everything that draws.

const loaded = await import('../engine/catalog.js').catch(() => null);
export const catalog = loaded?.default ?? null;

const KEY = 'clairlune-look';
const DEFAULTS = { palette: 'classic', letters: 'text', weather: 'off' };
const state = { ...DEFAULTS };
try {
  Object.assign(state, JSON.parse(localStorage.getItem(KEY) || '{}'));
} catch {}

const reduce = matchMedia('(prefers-reduced-motion: reduce)');
export const calm = () => reduce.matches;

export const events = new EventTarget();
export const look = () => ({ ...state });

export function setLook(patch, { persist = true } = {}) {
  const changed = Object.keys(patch).filter((k) => patch[k] !== state[k]);
  if (!changed.length) return;
  Object.assign(state, patch);
  apply();
  if (persist) {
    try {
      localStorage.setItem(KEY, JSON.stringify(state));
    } catch {}
  }
  events.dispatchEvent(new CustomEvent('change', { detail: { ...state, changed } }));
}

// --- Colour -----------------------------------------------------------------

const rgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
const toHex = (c) => '#' + c.map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');
const mix = (a, b, t) => toHex(rgb(a).map((v, i) => v + (rgb(b)[i] - v) * t));
function lum(hex) {
  const [r, g, b] = rgb(hex).map((v) => {
    v /= 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
const contrast = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};
// `c` moved toward `toward` until it reads against `bg`.
function readable(c, bg, toward, min = 4.5) {
  let t = 0;
  let out = c;
  while (contrast(out, bg) < min && t < 1) {
    t += 0.05;
    out = mix(c, toward, t);
  }
  return out;
}

// Site tokens for one palette family: the page drawn in the wallpaper's
// colours, with text kept readable.
function tokens(f, dark) {
  const bg = f.bg;
  const ink = readable(f.ink, bg, dark ? '#FFFFFF' : '#000000', 7);
  const accent = f.accent;
  const onAccent = contrast('#000000', accent) >= contrast('#FFFFFF', accent) ? '#14110E' : '#FFFFFF';
  return {
    bg,
    'bg-sidebar': mix(bg, f.edge, 0.7),
    'bg-raised': dark ? mix(bg, ink, 0.045) : mix(bg, '#FFFFFF', 0.55),
    hairline: mix(bg, ink, 0.13),
    'border-control': readable(mix(bg, ink, 0.45), bg, ink, 3),
    text: ink,
    'text-secondary': readable(mix(ink, bg, 0.32), bg, ink),
    'text-disabled': mix(ink, bg, 0.6),
    accent,
    'accent-deep': accent,
    'accent-text': readable(accent, bg, ink),
    'on-accent': onAccent,
  };
}

const TOKEN_NAMES = [
  'bg', 'bg-sidebar', 'bg-raised', 'hairline', 'border-control', 'text', 'text-secondary',
  'text-disabled', 'accent', 'accent-deep', 'accent-text', 'on-accent',
];

function applyPalette(id) {
  const root = document.documentElement.style;
  const p = catalog?.palettes.find((x) => x.id === id);
  for (const theme of ['light', 'dark']) {
    for (const n of TOKEN_NAMES) root.removeProperty(`--${n}-${theme}`);
  }
  document.documentElement.dataset.palette = p && id !== 'classic' ? id : '';
  const paint = {};
  if (p && id !== 'classic') {
    for (const theme of ['light', 'dark']) {
      const fam = p[theme];
      if (!fam) continue;
      for (const [n, v] of Object.entries(tokens(fam, theme === 'dark'))) paint[`--${n}-${theme}`] = v;
    }
  }
  for (const [k, v] of Object.entries(paint)) root.setProperty(k, v);
  // Every page paints these before it shows (a line in each page's head).
  try {
    if (Object.keys(paint).length) localStorage.setItem('clairlune-paint', JSON.stringify(paint));
    else localStorage.removeItem('clairlune-paint');
  } catch {}
}

// --- Letters ------------------------------------------------------------------

export function inLetters(text, set = state.letters) {
  if (!catalog || set === 'text') return text;
  const to = [...(catalog.letters.find((l) => l.id === set)?.chars ?? '')];
  const from = [...catalog.glyphs];
  return [...text].map((ch) => {
    const i = from.indexOf(ch);
    return i > 0 && to[i] ? to[i] : ch;
  }).join('');
}

function applyLetters(set) {
  for (const el of document.querySelectorAll('.ascii-art')) {
    el.dataset.text ??= el.textContent;
    el.textContent = inLetters(el.dataset.text, set);
  }
}

export function apply() {
  if (calm() && state.weather !== 'off') state.weather = 'off';
  applyPalette(state.palette);
  applyLetters(state.letters);
}

apply();
