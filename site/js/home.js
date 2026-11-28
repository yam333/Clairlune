// The homepage's live parts. Each starts on its own, so one that can't (an
// old browser, the engine failing to load) leaves the others working and
// the page as it was written.
import { ready } from './engine.js';
import { catalog, events, look, setLook, calm } from './look.js';

const status = document.getElementById('copy-status');
const announce = (text) => {
  if (status) status.textContent = text;
};

async function start(name, load) {
  try {
    await load();
  } catch (e) {
    console.warn(`clairlune: ${name} didn't start`, e);
  }
}

// The palette, letter and weather pickers in "Make it yours".
function initPickers() {
  const groups = document.querySelectorAll('[data-look]');
  const show = () => {
    const l = look();
    for (const g of groups) {
      const key = g.dataset.look;
      for (const b of g.querySelectorAll('button[data-value]')) {
        b.setAttribute('aria-pressed', String(b.dataset.value === l[key]));
        if (key === 'weather') b.disabled = calm() && b.dataset.value !== 'off';
      }
    }
  };
  for (const g of groups) {
    g.addEventListener('click', (e) => {
      const b = e.target.closest('button[data-value]');
      if (b && !b.disabled) setLook({ [g.dataset.look]: b.dataset.value });
    });
  }
  events.addEventListener('change', show);
  show();
  if (calm()) document.getElementById('weather-calm')?.removeAttribute('hidden');
}

start('pickers', async () => catalog && initPickers());
start('weather', async () => (await import('./weather.js')).initWeather());

ready.then(
  () => {
    document.documentElement.classList.add('engine-ready');
    start('hero', async () => (await import('./hero.js')).initHero({ announce }));
    start('desktop', async () => (await import('./desktop.js')).initDesktop());
    start('timeline', async () => (await import('./timeline.js')).initTimeline());
    start('terminal', async () => (await import('./terminal.js')).initTerminal({ announce }));
    start('clawd', async () => (await import('./clawd.js')).initClawd());
  },
  (e) => console.warn('clairlune: the engine did not load', e),
);
