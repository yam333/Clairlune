// A terminal that runs clairlune-cli's drawing commands in the page: the
// same engine, the same flags, the same answers. Commands that need a
// computer of their own (setting a wallpaper, GIFs) say how to get it.
import { View, png, save, randomSeed } from './engine.js';
import { catalog } from './look.js';

const HELP = [
  'clairlune-cli <command> [flags]',
  '',
  '  generate        a scene from a seed, as a PNG',
  '  contact-sheet   many seeds as small tiles',
  '  monitors        the screens it would draw for',
  '  clear           empty this terminal',
  '',
  'generate flags: --seed --theme light|dark --archetype --activity --weather',
  '                --palette --letters --look line|dither --zoom --no-buddy',
  '                --size WxH --out file.png',
  'Here it runs in your browser. On your computer: cargo install clairlune-cli',
];
const ELSEWHERE = {
  gif: 'GIFs are encoded on your computer: cargo install clairlune-cli',
  package: 'Packages are written to disk on your computer: cargo install clairlune-cli',
  set: 'Setting the wallpaper needs your computer: install Clairlune, or cargo install clairlune-cli',
  restore: 'Nothing to restore in a browser.',
  live: 'Live plays behind your desktop icons: install Clairlune for that.',
  'preview-activity': 'The activity previewer opens a window on your computer: cargo install clairlune-cli',
};
const FLAGS = new Set([
  'seed', 'theme', 'archetype', 'activity', 'weather', 'palette', 'letters', 'look', 'zoom',
  'contrast', 'size', 'out', 'count', 'from', 'columns', 'tile', 'time-ms', 'recipe-out',
]);
const MAX_LOG = 8;

function parse(line) {
  const words = line.trim().split(/\s+/).filter(Boolean);
  if (words[0] === 'clairlune-cli') words.shift();
  const command = words.shift() || '';
  const flags = {};
  const extra = [];
  for (let i = 0; i < words.length; i++) {
    const w = words[i];
    if (w === '--no-buddy') flags['no-buddy'] = true;
    else if (w === '--help' || w === '-h') flags.help = true;
    else if (w.startsWith('--')) {
      const [name, inline] = w.slice(2).split('=');
      if (!FLAGS.has(name)) throw new Error(`unexpected argument '--${name}'`);
      const v = inline ?? words[++i];
      if (v === undefined) throw new Error(`a value is required for '--${name}'`);
      flags[name] = v;
    } else extra.push(w);
  }
  if (extra.length) throw new Error(`unexpected argument '${extra[0]}'`);
  return { command, flags };
}

function sceneOpts(f) {
  if (f.theme && !['light', 'dark'].includes(f.theme)) {
    throw new Error(`invalid value '${f.theme}' for '--theme': light or dark`);
  }
  if (f.seed !== undefined && !/^\d{1,20}$/.test(f.seed)) {
    throw new Error(`invalid value '${f.seed}' for '--seed': a whole number`);
  }
  return {
    seed: f.seed ?? randomSeed(),
    theme: f.theme || 'dark',
    landscape: f.archetype || null,
    activity: f.activity || null,
    weather: f.weather || null,
    palette: f.palette || null,
    letters: f.letters || null,
    look: f.look || null,
    zoom: f.zoom || null,
    contrast: f.contrast || null,
    buddy: !f['no-buddy'],
  };
}

function size(text, fallback) {
  if (!text) return fallback;
  const m = /^(\d{2,5})x(\d{2,5})$/.exec(text);
  if (!m) throw new Error(`invalid value '${text}' for '--size': WIDTHxHEIGHT`);
  return [Number(m[1]), Number(m[2])];
}

export function initTerminal({ announce }) {
  const term = document.getElementById('term');
  if (!term || !catalog) return;
  const log = term.querySelector('.term-log');
  const form = term.querySelector('form');
  const input = form.querySelector('input');
  const history = [];
  let back = 0;
  const entries = [];

  function entry(line) {
    const e = document.createElement('div');
    e.className = 'term-entry';
    const echo = document.createElement('div');
    echo.className = 'term-echo';
    echo.textContent = `$ ${line}`;
    e.append(echo);
    log.append(e);
    entries.push(e);
    while (entries.length > MAX_LOG) {
      const old = entries.shift();
      old.views?.forEach((v) => v.free());
      old.remove();
    }
    return e;
  }

  function say(e, text, cls = '') {
    const p = document.createElement('pre');
    p.className = `term-out ${cls}`;
    p.textContent = text;
    e.append(p);
    return p;
  }

  function scroll() {
    log.scrollTop = log.scrollHeight;
  }

  async function run(line) {
    if (!line.trim()) return;
    history.push(line);
    back = history.length;
    if (line.trim() === 'clear') {
      for (const e of entries.splice(0)) {
        e.views?.forEach((v) => v.free());
        e.remove();
      }
      return;
    }
    const e = entry(line);
    e.views = [];
    try {
      const { command, flags } = parse(line);
      if (!command || command === 'help' || flags.help) {
        say(e, HELP.join('\n'));
      } else if (command === 'generate') {
        const o = sceneOpts(flags);
        const [W, H] = size(flags.size, [1920, 1080]);
        const out = /\.png$/i.test(flags.out || '') ? flags.out : 'wallpaper.png';
        const fig = document.createElement('div');
        fig.className = 'term-figure';
        const c = document.createElement('canvas');
        fig.append(c);
        e.append(fig);
        const v = new View(c, { still: true, aspect: W / H, maxWidth: 1200 });
        e.views.push(v);
        const info = await v.set(o);
        const p = say(e, `seed ${info.seed} · ${info.title} · ${info.theme}\nwrote `, 'term-ok');
        const a = document.createElement('button');
        a.type = 'button';
        a.className = 'term-file';
        a.textContent = `${out} (${W}x${H})`;
        a.addEventListener('click', async () => {
          a.disabled = true;
          try {
            const { blob } = await png({ ...o, seed: info.seed, width: W, height: H });
            save(blob, out);
            announce(`${out} saved`);
          } catch (err) {
            say(e, `error: ${err.message}`, 'term-err');
          } finally {
            a.disabled = false;
          }
        });
        p.append(a);
      } else if (command === 'contact-sheet') {
        const n = Math.min(12, Math.max(1, Number(flags.count || 8)));
        const from = BigInt(/^\d+$/.test(flags.from || '') ? flags.from : '1');
        const grid = document.createElement('div');
        grid.className = 'term-sheet';
        e.append(grid);
        const base = sceneOpts(flags);
        for (let i = 0n; i < BigInt(n); i++) {
          const cell = document.createElement('figure');
          const c = document.createElement('canvas');
          const cap = document.createElement('figcaption');
          cell.append(c, cap);
          grid.append(cell);
          const v = new View(c, { still: true, maxWidth: 480 });
          e.views.push(v);
          // As the real contact sheet: a scene of the other time of day is shown then.
          const info = await v.set({ ...base, seed: String(from + i), fallback: true });
          cap.textContent = `${info.seed} · ${info.landscapeName}`;
          scroll();
        }
        say(e, `${n} seeds from ${from}`, 'term-ok');
      } else if (command === 'monitors') {
        const d = window.devicePixelRatio || 1;
        say(e, `browser  ${Math.round(screen.width * d)}x${Math.round(screen.height * d)}  (main, ${d}x)\nOn your computer it lists every monitor and its wallpaper.`);
      } else if (ELSEWHERE[command]) {
        say(e, ELSEWHERE[command]);
      } else {
        say(e, `error: unrecognized subcommand '${command}'\n\nFor more information, try 'help'.`, 'term-err');
      }
    } catch (err) {
      say(e, `error: ${err.message}`, 'term-err');
    }
    scroll();
  }

  form.addEventListener('submit', (ev) => {
    ev.preventDefault();
    const line = input.value;
    input.value = '';
    run(line);
  });
  input.addEventListener('keydown', (ev) => {
    if (ev.key === 'ArrowUp' && back > 0) {
      ev.preventDefault();
      input.value = history[--back];
    } else if (ev.key === 'ArrowDown') {
      ev.preventDefault();
      back = Math.min(history.length, back + 1);
      input.value = history[back] ?? '';
    }
  });
  for (const b of document.querySelectorAll('[data-term]')) {
    b.addEventListener('click', () => {
      input.value = '';
      run(b.dataset.term);
      input.focus({ preventScroll: true });
    });
  }
  term.addEventListener('click', (ev) => {
    if (!ev.target.closest('button, a, canvas') && !getSelection()?.toString()) input.focus({ preventScroll: true });
  });
  run('clairlune-cli generate --seed 7 --archetype mountains --activity warm_hands');
}
