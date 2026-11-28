// The hero: the real engine drawing a scene you choose. A seed of the day
// for everyone, a link for any scene, and a PNG sized to your screen.
import { View, png, save, seedOfDay, randomSeed, prefersStill } from './engine.js';
import { catalog, events, look, setLook } from './look.js';

const $ = (id) => document.getElementById(id);

// Day from 06:00 to 18:59 by the visitor's own clock.
export const skyNow = () => {
  const h = new Date().getHours();
  return h >= 6 && h < 19 ? 'light' : 'dark';
};

const hhmm = (d = new Date()) => d.toTimeString().slice(0, 5);

export function initHero({ announce }) {
  const canvas = $('stage-canvas');
  if (!canvas || !catalog) return null;
  const stage = $('stage');
  const controls = $('play-controls');
  const seedInput = $('seed-input');
  const land = $('pick-land');
  const pal = $('pick-palette');
  const lett = $('pick-letters');
  const sky = $('pick-sky');
  const clawd = $('clawd-toggle');
  const live = $('live-toggle');
  const title = $('menubar-scene');
  const clock = $('menubar-clock');
  const cmd = $('stage-cmd');
  const note = $('stage-note');
  const today = $('seed-today');
  const download = $('download-png');

  const option = (sel, value, label) => sel.append(new Option(label, value));
  option(land, '', 'Any');
  for (const l of [...catalog.landscapes].sort((a, b) => a.name.localeCompare(b.name))) option(land, l.id, l.name);
  for (const p of catalog.palettes) option(pal, p.id, p.name);
  for (const l of catalog.letters) option(lett, l.id, l.name);

  // What's showing: from the link, else today's seed and the visitor's look.
  const q = new URLSearchParams(location.search);
  const s = {
    seed: /^\d{1,20}$/.test(q.get('seed') || '') ? q.get('seed') : seedOfDay(),
    land: q.get('land') || '',
    sky: ['day', 'night'].includes(q.get('sky')) ? q.get('sky') : 'clock',
    clawd: q.get('clawd') !== '0',
  };
  if (!catalog.landscapes.some((l) => l.id === s.land)) s.land = '';
  const shared = {};
  if (catalog.palettes.some((p) => p.id === q.get('palette'))) shared.palette = q.get('palette');
  if (catalog.letters.some((l) => l.id === q.get('letters'))) shared.letters = q.get('letters');
  if (Object.keys(shared).length) setLook(shared, { persist: false });

  const view = new View(canvas, { fps: 20, maxWidth: 2400 });
  let playing = !prefersStill();
  let info = null;
  let busy = 0;

  const theme = () => (s.sky === 'day' ? 'light' : s.sky === 'night' ? 'dark' : skyNow());

  // As clairlune-cli takes them: only what was chosen, defaults left out.
  function opts() {
    const l = look();
    return {
      seed: s.seed,
      theme: theme(),
      landscape: s.land || null,
      palette: l.palette === 'classic' ? null : l.palette,
      letters: l.letters === 'text' ? null : l.letters,
      buddy: s.clawd,
      fallback: true,
    };
  }

  // The command that draws this very scene (1920x1080 is the CLI's size
  // and the stage's shape).
  function cli(o, shown) {
    let c = `clairlune-cli generate --seed ${o.seed} --theme ${shown || o.theme}`;
    if (o.landscape) c += ` --archetype ${o.landscape}`;
    if (o.palette) c += ` --palette ${o.palette}`;
    if (o.letters) c += ` --letters ${o.letters}`;
    if (!o.buddy) c += ' --no-buddy';
    return c + ' --out wallpaper.png';
  }

  function link() {
    const o = opts();
    const p = new URLSearchParams({ seed: s.seed });
    if (s.land) p.set('land', s.land);
    if (o.palette) p.set('palette', o.palette);
    if (o.letters) p.set('letters', o.letters);
    if (s.sky !== 'clock') p.set('sky', s.sky);
    if (!s.clawd) p.set('clawd', '0');
    return `${location.origin}${location.pathname}?${p}`;
  }

  function sync() {
    const l = look();
    seedInput.value = s.seed;
    land.value = s.land;
    pal.value = l.palette;
    lett.value = l.letters;
    sky.value = s.sky;
    clawd.checked = s.clawd;
    today.setAttribute('aria-pressed', String(s.seed === seedOfDay()));
    live.textContent = playing ? 'Pause' : 'Play';
  }

  function label() {
    if (!info) return;
    title.textContent = `${info.title} · ${playing ? 'LIVE' : 'STILL'}`;
    canvas.setAttribute('aria-label', `${info.title}, seed ${info.seed}, drawn by Clairlune's engine`);
  }

  async function draw({ restart = true, remember = false } = {}) {
    const n = ++busy;
    const o = opts();
    sync();
    cmd.textContent = cli(o);
    try {
      const got = await view.set(o, { play: playing, restart });
      if (n !== busy) return;
      info = got;
      cmd.textContent = cli(o, got.theme);
      note.textContent =
        got.theme === o.theme
          ? ''
          : `This scene only happens ${got.theme === 'dark' ? 'at night' : 'by day'}.`;
      stage.classList.add('is-live');
      label();
      if (remember) history.replaceState(null, '', link());
    } catch (e) {
      if (n !== busy) return;
      note.textContent = e.message;
    }
  }

  seedInput.closest('form').addEventListener('submit', (e) => {
    e.preventDefault();
    const v = seedInput.value.trim();
    if (!/^\d{1,20}$/.test(v)) {
      note.textContent = 'A seed is a whole number of up to 20 digits.';
      return;
    }
    s.seed = v.replace(/^0+(?=\d)/, '');
    draw({ remember: true });
  });
  seedInput.addEventListener('change', () => seedInput.form.requestSubmit());
  $('seed-shuffle').addEventListener('click', () => {
    s.seed = randomSeed();
    draw({ remember: true });
  });
  today.addEventListener('click', () => {
    s.seed = seedOfDay();
    draw({ remember: true });
  });
  land.addEventListener('change', () => {
    s.land = land.value;
    draw({ remember: true });
  });
  sky.addEventListener('change', () => {
    s.sky = sky.value;
    draw({ remember: true });
  });
  clawd.addEventListener('change', () => {
    s.clawd = clawd.checked;
    draw({ remember: true });
  });
  pal.addEventListener('change', () => setLook({ palette: pal.value }));
  lett.addEventListener('change', () => setLook({ letters: lett.value }));
  events.addEventListener('change', (e) => {
    if (e.detail.changed.some((k) => k === 'palette' || k === 'letters')) draw({ remember: true });
  });
  live.addEventListener('click', () => {
    playing = !playing;
    if (playing) view.play();
    else view.pause();
    sync();
    label();
  });
  $('copy-link').addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(link());
      announce('Link copied');
    } catch {
      announce('Could not copy the link');
    }
  });

  // A wallpaper for the screen this page is on, at its real pixels.
  const dpr = Math.min(window.devicePixelRatio || 1, 3);
  let W = Math.round(screen.width * dpr);
  let H = Math.round(screen.height * dpr);
  const cap = 6144 * 3456;
  if (W * H > cap) {
    const k = Math.sqrt(cap / (W * H));
    W = Math.floor(W * k);
    H = Math.floor(H * k);
  }
  download.textContent = `Download ${W}×${H}`;
  download.addEventListener('click', async () => {
    if (download.disabled) return;
    download.disabled = true;
    const was = download.textContent;
    download.textContent = 'Drawing…';
    try {
      const { blob } = await png({ ...opts(), width: W, height: H });
      save(blob, `clairlune-${s.seed}-${W}x${H}.png`);
      announce('Wallpaper saved');
    } catch (e) {
      note.textContent = e.message;
    } finally {
      download.disabled = false;
      download.textContent = was;
    }
  });

  // The menu bar's clock, and the sky turning at 06:00 and 19:00.
  let shownSky = theme();
  const tick = () => {
    clock.textContent = hhmm();
    if (s.sky === 'clock' && theme() !== shownSky) {
      shownSky = theme();
      draw({ restart: false });
    }
  };
  tick();
  setInterval(tick, 15000);

  controls.hidden = false;
  draw();
  return { link };
}
