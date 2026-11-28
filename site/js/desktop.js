// A desktop to try Live's manners on: the wallpaper plays while you can see
// it, rests behind a window that fills the screen (on a Mac the menu bar
// turns solid then, as Clairlune makes it), and rests while the screen is
// locked. The same rule as the app: a window covering 92% of the screen.
import { View, seedOfDay, prefersStill } from './engine.js';
import { events, look } from './look.js';
import { skyNow } from './hero.js';

const FILLED = 0.92;
const $ = (id) => document.getElementById(id);
const hhmm = () => new Date().toTimeString().slice(0, 5);

export function initDesktop() {
  const sim = $('sim');
  if (!sim) return;
  const canvas = $('sim-wallpaper');
  const win = $('sim-window');
  const bar = win.querySelector('.sim-titlebar');
  const lock = $('sim-lock');
  const liveOut = $('desk-live');
  const barOut = $('desk-bar');
  const reopen = $('sim-reopen');
  const lockBtn = $('sim-lock-btn');
  const maxBtn = $('sim-max-btn');
  const osButtons = [...document.querySelectorAll('[data-sim-os]')];

  // Window frame in % of the desktop.
  const restored = { x: 20, y: 15, w: 46, h: 54 };
  let frame = { ...restored };
  let maximized = false;
  let open = true;
  let locked = false;
  let os = /Mac|iPhone|iPad/.test(navigator.platform) ? 'mac' : 'windows';
  const still = prefersStill();

  const view = new View(canvas, { fps: 12, aspect: 16 / 10, maxWidth: 1800 });
  const scene = () => {
    const l = look();
    return {
      seed: String((Number(seedOfDay()) + 1) % 1000000),
      theme: skyNow(),
      landscape: 'snowfield',
      weather: 'snow',
      palette: l.palette === 'classic' ? null : l.palette,
      letters: l.letters === 'text' ? null : l.letters,
      fallback: true,
    };
  };
  view.set(scene(), { play: !still }).catch(() => {});
  events.addEventListener('change', (e) => {
    if (e.detail.changed.some((k) => k === 'palette' || k === 'letters')) {
      view.set(scene(), { play: !still && !paused().now, restart: false }).catch(() => {});
    }
  });

  // The part of the screen apps can use: under the menu bar, above the taskbar.
  const work = () => (os === 'mac' ? { x: 0, y: 4.2, w: 100, h: 95.8 } : { x: 0, y: 0, w: 100, h: 91.5 });

  function coverage() {
    if (!open) return 0;
    const a = work();
    const x = Math.max(0, Math.min(frame.x + frame.w, a.x + a.w) - Math.max(frame.x, a.x));
    const y = Math.max(0, Math.min(frame.y + frame.h, a.y + a.h) - Math.max(frame.y, a.y));
    return (x * y) / (a.w * a.h);
  }

  function paused() {
    const covered = coverage() >= FILLED;
    return { now: locked || covered, covered };
  }

  let wasPaused = null;
  function render() {
    sim.dataset.os = os;
    for (const b of osButtons) b.setAttribute('aria-pressed', String(b.dataset.simOs === os));
    Object.assign(win.style, {
      left: `${frame.x}%`,
      top: `${frame.y}%`,
      width: `${frame.w}%`,
      height: `${frame.h}%`,
    });
    win.hidden = !open;
    reopen.hidden = open;
    lock.hidden = !locked;
    const p = paused();
    sim.classList.toggle('is-covered', p.covered);
    sim.classList.toggle('is-maximized', maximized && open);
    sim.classList.toggle('is-locked', locked);
    liveOut.textContent = still
      ? 'Still (your system asks for less motion)'
      : locked
        ? 'Paused while locked'
        : p.covered
          ? 'Paused: a window fills the screen'
          : 'Playing';
    barOut.textContent = p.covered ? 'Solid' : 'See-through';
    barOut.parentElement.hidden = os !== 'mac';
    maxBtn.textContent = maximized ? 'Restore the window' : 'Maximize the window';
    maxBtn.disabled = !open;
    lockBtn.textContent = locked ? 'Unlock' : 'Lock the screen';
    if (!still && p.now !== wasPaused) {
      if (p.now) view.pause();
      else view.play();
    }
    wasPaused = p.now;
  }

  function toggleMax() {
    if (!open) return;
    if (maximized) {
      frame = { ...restored };
    } else {
      Object.assign(restored, frame);
      const a = work();
      frame = { ...a };
    }
    maximized = !maximized;
    render();
  }

  // Dragging the window by its title bar.
  bar.addEventListener('pointerdown', (e) => {
    if (e.target.closest('button')) return;
    const r = sim.getBoundingClientRect();
    const start = { x: e.clientX, y: e.clientY, fx: frame.x, fy: frame.y };
    bar.setPointerCapture(e.pointerId);
    if (maximized) {
      // Pulling a maximized window down restores it under the pointer.
      maximized = false;
      frame = { ...restored, x: ((e.clientX - r.left) / r.width) * 100 - restored.w / 2, y: frame.y };
      start.fx = frame.x;
      start.fy = frame.y;
    }
    const move = (ev) => {
      const a = work();
      frame.x = Math.min(100 - 8, Math.max(-frame.w + 8, start.fx + ((ev.clientX - start.x) / r.width) * 100));
      frame.y = Math.min(100 - 8, Math.max(a.y, start.fy + ((ev.clientY - start.y) / r.height) * 100));
      render();
    };
    const up = () => {
      bar.removeEventListener('pointermove', move);
      bar.removeEventListener('pointerup', up);
      bar.removeEventListener('pointercancel', up);
    };
    bar.addEventListener('pointermove', move);
    bar.addEventListener('pointerup', up);
    bar.addEventListener('pointercancel', up);
  });
  bar.addEventListener('dblclick', (e) => {
    if (!e.target.closest('button')) toggleMax();
  });
  // Resizing from the corner.
  win.querySelector('.sim-grip').addEventListener('pointerdown', (e) => {
    e.stopPropagation();
    const r = sim.getBoundingClientRect();
    const start = { x: e.clientX, y: e.clientY, w: frame.w, h: frame.h };
    const grip = e.currentTarget;
    grip.setPointerCapture(e.pointerId);
    maximized = false;
    const move = (ev) => {
      frame.w = Math.min(100 - frame.x, Math.max(22, start.w + ((ev.clientX - start.x) / r.width) * 100));
      frame.h = Math.min(100 - frame.y, Math.max(22, start.h + ((ev.clientY - start.y) / r.height) * 100));
      render();
    };
    const up = () => grip.removeEventListener('pointermove', move);
    grip.addEventListener('pointermove', move);
    grip.addEventListener('pointerup', up, { once: true });
  });

  win.querySelectorAll('[data-win]').forEach((b) =>
    b.addEventListener('click', () => {
      const what = b.dataset.win;
      if (what === 'max') toggleMax();
      else {
        open = false;
        maximized = false;
        frame = { ...restored };
        render();
      }
    }),
  );
  const show = () => {
    open = true;
    render();
  };
  reopen.addEventListener('click', show);
  sim.querySelectorAll('.sim-icon').forEach((i) => i.addEventListener('dblclick', show));
  maxBtn.addEventListener('click', toggleMax);
  lockBtn.addEventListener('click', () => {
    locked = !locked;
    render();
  });
  lock.addEventListener('click', () => {
    locked = false;
    render();
  });
  for (const b of osButtons) {
    b.addEventListener('click', () => {
      os = b.dataset.simOs;
      if (maximized) frame = { ...work() };
      render();
    });
  }

  const clocks = sim.querySelectorAll('[data-sim-clock]');
  const tick = () => clocks.forEach((c) => (c.textContent = hhmm()));
  tick();
  setInterval(tick, 15000);
  render();
}
