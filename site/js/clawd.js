// Clawd, in his own pixels from the app's sprite sheets, walks along the
// line above each section's label as you scroll through it, and stops to
// do something when you do. A click makes him celebrate.
import { clawd as frames, prefersStill } from './engine.js';

const SCALE = 2;
// What he does when he first stops in a section (then he idles).
const MOODS = [null, 'alert', 'celebrate', 'alert', null, 'celebrate', 'alert'];

export async function initClawd() {
  const kickers = [...document.querySelectorAll('main > section > .section-kicker')];
  if (!kickers.length) return;
  const sheets = {};
  for (const s of ['idle', 'walk', 'celebrate', 'alert']) {
    const f = await frames(s);
    if (!f.length) return;
    sheets[s] = f.map((fr) => {
      const c = document.createElement('canvas');
      c.width = fr.w;
      c.height = fr.h;
      c.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(fr.rgba), fr.w, fr.h), 0, 0);
      return { ...fr, c };
    });
  }
  const W = Math.max(...Object.values(sheets).flat().map((f) => f.w));
  const H = Math.max(...Object.values(sheets).flat().map((f) => f.h));

  const el = document.createElement('canvas');
  el.className = 'clawd-walker';
  el.width = W * SCALE;
  el.height = H * SCALE;
  el.setAttribute('aria-hidden', 'true');
  el.title = 'Clawd';
  const ctx = el.getContext('2d');
  ctx.imageSmoothingEnabled = false;

  const still = prefersStill();
  let section = -1;
  let x = 0;
  let target = 0;
  let facing = 1;
  let state = 'idle';
  let stateAt = performance.now();
  let lastMove = 0;
  let oneShot = null;

  function draw(now) {
    const sheet = sheets[oneShot || state];
    const total = sheet.reduce((a, f) => a + f.ms, 0) || 1;
    let t = (now - stateAt) % total;
    if (oneShot && now - stateAt >= total) {
      oneShot = null;
      stateAt = now;
      return draw(now);
    }
    let f = sheet[0];
    for (const fr of sheet) {
      if (t < fr.ms) {
        f = fr;
        break;
      }
      t -= fr.ms;
    }
    ctx.clearRect(0, 0, el.width, el.height);
    ctx.save();
    // Feet on the bottom edge, centred.
    const dx = (W / 2 - f.ox) * SCALE;
    const dy = (H - 1 - f.oy) * SCALE;
    if (facing < 0) {
      ctx.translate(el.width, 0);
      ctx.scale(-1, 1);
    }
    ctx.drawImage(f.c, dx, dy, f.w * SCALE, f.h * SCALE);
    ctx.restore();
  }

  function setState(s, now) {
    if (s !== state) {
      state = s;
      stateAt = now;
    }
  }

  // Which section is most in view, and how far through it the reader is.
  function where() {
    const vh = innerHeight;
    let best = -1;
    let bestSeen = 0;
    let progress = 0;
    kickers.forEach((k, i) => {
      const sec = k.parentElement.getBoundingClientRect();
      const seen = Math.min(sec.bottom, vh) - Math.max(sec.top, 0);
      if (seen > bestSeen) {
        bestSeen = seen;
        best = i;
        progress = Math.min(1, Math.max(0, (vh * 0.5 - sec.top) / Math.max(1, sec.height)));
      }
    });
    return { i: best, progress };
  }

  let greeted = -1;
  function place(now) {
    let { i, progress } = where();
    if (i < 0) return;
    // The hero's line is under the sticky header: wait on the next one.
    if (i === 0 && kickers.length > 1) {
      i = 1;
      progress = 0.08;
    }
    const k = kickers[i];
    if (i !== section) {
      section = i;
      k.append(el);
      x = 24 + progress * Math.max(0, k.clientWidth - el.width - 48);
      lastMove = now;
    }
    const span = Math.max(0, k.clientWidth - el.width - 48);
    target = 24 + progress * span;
    if (still) x = target;
  }

  let last = performance.now();
  function frame(now) {
    requestAnimationFrame(frame);
    const dt = Math.min(now - last, 100);
    last = now;
    place(now);
    const gap = target - x;
    if (!still && Math.abs(gap) > 1.5) {
      const speed = 0.09 * SCALE;
      const step = Math.sign(gap) * Math.min(Math.abs(gap), speed * dt);
      x += step;
      facing = step >= 0 ? 1 : -1;
      lastMove = now;
      if (!oneShot) setState('walk', now);
    } else if (!oneShot && now - lastMove > 350) {
      if (greeted !== section && MOODS[section] && !still) {
        greeted = section;
        oneShot = MOODS[section];
        stateAt = now;
      } else {
        setState('idle', now);
      }
    }
    el.style.transform = `translateX(${Math.round(x)}px)`;
    draw(now);
  }

  el.addEventListener('click', () => {
    oneShot = 'celebrate';
    stateAt = performance.now();
  });
  requestAnimationFrame(frame);
}
