// The daily schedule card, live: drag where each slot starts (or use the
// arrow keys), the "now" line follows your clock, and the slot playing now
// shows in a small preview drawn by the engine.
import { View, prefersStill } from './engine.js';
import { events, look } from './look.js';

const SLOTS = [
  { land: 'observatory', name: 'Observatory', theme: 'dark', live: true, seed: '12' },
  { land: 'meadow', name: 'Meadow', theme: 'light', live: false, seed: '31' },
  { land: 'fjord', name: 'Fjord', theme: 'light', live: true, seed: '7' },
  { land: 'lantern_lake', name: 'Lantern lake', theme: 'dark', live: true, seed: '5' },
];
const DAY = 1440;
const STEP = 30;
const MIN = 60;
const fmt = (m) => `${String(Math.floor(m / 60) % 24).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
const fmtEnd = (m) => (m === DAY ? '24:00' : fmt(m));

export function initTimeline() {
  const card = document.getElementById('tl-card');
  if (!card) return;
  const track = card.querySelector('.timeline-track');
  const segs = [...track.querySelectorAll('.tl-seg')];
  const nowLine = track.querySelector('.tl-now');
  const handles = [...card.querySelectorAll('.tl-handle')];
  const ticks = card.querySelector('.timeline-ticks');
  const rows = [...card.querySelectorAll('.tl-row')];
  const status = card.querySelector('.tl-status-text');
  const canvas = card.querySelector('.tl-preview canvas');
  const previewLabel = card.querySelector('.tl-preview-label');

  // Where slots 1..3 start (slot 0 starts at midnight).
  const starts = [0, 450, 780, 1110];
  let picked = null;

  const view = canvas ? new View(canvas, { fps: 12, aspect: 3, maxWidth: 1200 }) : null;
  let shown = '';

  const minutesNow = () => {
    const d = new Date();
    return d.getHours() * 60 + d.getMinutes();
  };
  const slotAt = (m) => {
    let i = 0;
    for (let k = 0; k < starts.length; k++) if (m >= starts[k]) i = k;
    return i;
  };

  function preview(i, isNow) {
    if (!view) return;
    const s = SLOTS[i];
    const l = look();
    const key = `${i}|${l.palette}|${l.letters}`;
    previewLabel.textContent = `${isNow ? 'Now' : fmt(starts[i])} · ${s.name} · ${s.theme === 'dark' ? 'Dark' : 'Light'} · ${s.live ? 'Live' : 'Still'}`;
    if (key === shown) return;
    shown = key;
    view
      .set(
        {
          seed: s.seed,
          landscape: s.land,
          theme: s.theme,
          palette: l.palette === 'classic' ? null : l.palette,
          letters: l.letters === 'text' ? null : l.letters,
          fallback: true,
        },
        { play: s.live && !prefersStill() },
      )
      .catch(() => {});
  }

  function render() {
    const ends = [...starts.slice(1), DAY];
    track.style.gridTemplateColumns = starts.map((s, i) => `${ends[i] - s}fr`).join(' ');
    const now = minutesNow();
    const active = slotAt(now);
    nowLine.style.left = `${(now / DAY) * 100}%`;
    segs.forEach((seg, i) => seg.classList.toggle('tl-seg-active', i === active));
    handles.forEach((h, k) => {
      const i = k + 1;
      const v = starts[i];
      h.style.left = `${(v / DAY) * 100}%`;
      h.setAttribute('aria-valuenow', String(v));
      h.setAttribute('aria-valuemin', String(starts[i - 1] + MIN));
      h.setAttribute('aria-valuemax', String((starts[i + 1] ?? DAY) - MIN));
      h.setAttribute('aria-valuetext', `${SLOTS[i].name} from ${fmt(v)}`);
    });
    ticks.innerHTML = '';
    for (const m of [...starts, DAY]) {
      const t = document.createElement('span');
      t.textContent = fmtEnd(m);
      t.style.left = `${(m / DAY) * 100}%`;
      ticks.append(t);
    }
    rows.forEach((row, i) => {
      row.querySelector('.tl-time').textContent = `${fmt(starts[i])}–${fmtEnd(ends[i])}`;
      const isNow = i === active;
      row.classList.toggle('tl-row-active', isNow);
      row.setAttribute('aria-current', isNow ? 'time' : 'false');
      row.querySelector('.tl-mode').hidden = isNow;
      row.querySelector('.tl-badge').hidden = !isNow;
      row.querySelector('.tl-badge').textContent = `Now · ${SLOTS[i].live ? 'Live' : 'Still'}`;
      row.setAttribute('aria-pressed', String(picked === i));
    });
    status.textContent = `${fmt(now)} · ${SLOTS[active].name}`;
    preview(picked ?? active, picked === null);
  }

  const snap = (m) => Math.round(m / STEP) * STEP;
  function move(k, m) {
    const i = k + 1;
    starts[i] = Math.min((starts[i + 1] ?? DAY) - MIN, Math.max(starts[i - 1] + MIN, snap(m)));
    render();
  }

  handles.forEach((h, k) => {
    h.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      h.setPointerCapture(e.pointerId);
      h.focus();
      const r = track.getBoundingClientRect();
      const go = (ev) => move(k, ((ev.clientX - r.left) / r.width) * DAY);
      const up = () => {
        h.removeEventListener('pointermove', go);
        h.classList.remove('is-dragging');
      };
      h.classList.add('is-dragging');
      h.addEventListener('pointermove', go);
      h.addEventListener('pointerup', up, { once: true });
      h.addEventListener('pointercancel', up, { once: true });
    });
    h.addEventListener('keydown', (e) => {
      const i = k + 1;
      const d = { ArrowLeft: -STEP, ArrowDown: -STEP, ArrowRight: STEP, ArrowUp: STEP, PageDown: -120, PageUp: 120 }[e.key];
      if (d) {
        e.preventDefault();
        move(k, starts[i] + d);
      } else if (e.key === 'Home') {
        e.preventDefault();
        move(k, 0);
      } else if (e.key === 'End') {
        e.preventDefault();
        move(k, DAY);
      }
    });
  });

  rows.forEach((row, i) =>
    row.addEventListener('click', () => {
      picked = picked === i ? null : i;
      render();
    }),
  );

  events.addEventListener('change', (e) => {
    if (e.detail.changed.some((k) => k === 'palette' || k === 'letters')) render();
  });
  render();
  setInterval(render, 30000);
}
