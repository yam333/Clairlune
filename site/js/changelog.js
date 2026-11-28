// The changelog's pictures: a scene for each release, drawn by the engine
// when it scrolls into view. Releases that added landscapes show one of
// them; the seed is the version's own number (0.36.1 is seed 3601).
import { View, ready } from './engine.js';
import { look } from './look.js';

await ready;
document.documentElement.classList.add('engine-ready');
const l = look();
const io = new IntersectionObserver(
  (entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      io.unobserve(e.target);
      const c = e.target;
      const v = new View(c, { still: true, maxWidth: 720 });
      v.set({
        seed: c.dataset.seed,
        landscape: c.dataset.land || null,
        theme: c.dataset.theme || 'dark',
        palette: l.palette === 'classic' ? null : l.palette,
        letters: l.letters === 'text' ? null : l.letters,
        fallback: true,
      })
        .then((info) => {
          c.closest('figure').classList.add('is-drawn');
          const cap = c.closest('figure').querySelector('figcaption');
          if (cap) cap.textContent = `${info.title} · seed ${info.seed}`;
        })
        .catch(() => c.closest('figure').remove());
    }
  },
  { rootMargin: '300px 0px' },
);
for (const c of document.querySelectorAll('.release-scene canvas')) io.observe(c);
