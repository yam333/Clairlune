// Clairlune static landing page interactivity:
// 1. Two-state theme comfort override (`dark-mode-toggle` pattern)
// 2. Accessible OS install tabs (`tabs` pattern + auto-OS initial selection)
// 3. Genuine Clairlune engine specimen stage + 12 fps biome particle canvas
// 4. Docs pages: contents rail (folded on narrow screens, current section marked)

(() => {
  'use strict';

  // --- 1. Dark / Light Comfort Toggle ---
  const metaScheme = document.querySelector('meta[name="color-scheme"]');
  const themeBtn = document.getElementById('theme-toggle');
  const darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

  function currentResolvedScheme() {
    const content = metaScheme ? metaScheme.content : 'light dark';
    if (content === 'dark' || content === 'light') return content;
    return darkQuery.matches ? 'dark' : 'light';
  }

  if (themeBtn && metaScheme) {
    themeBtn.addEventListener('click', () => {
      const target = currentResolvedScheme() === 'dark' ? 'light' : 'dark';
      const systemScheme = darkQuery.matches ? 'dark' : 'light';
      try {
        if (target === systemScheme) {
          localStorage.removeItem('color-scheme');
          metaScheme.content = 'light dark';
        } else {
          localStorage.setItem('color-scheme', target);
          metaScheme.content = target;
        }
      } catch {
        metaScheme.content = target === systemScheme ? 'light dark' : target;
      }
    });

    window.addEventListener('storage', (e) => {
      if (e.key === 'color-scheme') {
        metaScheme.content = e.newValue ?? 'light dark';
      }
    });
  }

  // --- 2. OS Install Tablist (Selection Follows Focus + Find-in-Page Sync) ---
  const tablist = document.querySelector('.install-tablist');
  if (tablist) {
    const tabs = Array.from(tablist.querySelectorAll('[role="tab"]'));
    const panels = tabs.map((t) =>
      document.querySelector(`[role="tabpanel"][aria-labelledby="${t.id}"]`)
    );

    const selectTab = (nextTab, moveFocus = false) => {
      tabs.forEach((t, idx) => {
        const active = t === nextTab;
        t.setAttribute('aria-selected', String(active));
        t.tabIndex = active ? 0 : -1;
        const panel = panels[idx];
        if (panel) {
          if (active) {
            panel.removeAttribute('hidden');
          } else {
            panel.setAttribute('hidden', 'until-found');
          }
        }
      });
      if (moveFocus) nextTab.focus();
    };

    tabs.forEach((tab, idx) => {
      tab.addEventListener('click', () => selectTab(tab, false));
      tab.addEventListener('keydown', (e) => {
        let nextIdx = -1;
        if (e.key === 'ArrowRight') nextIdx = (idx + 1) % tabs.length;
        else if (e.key === 'ArrowLeft') nextIdx = (idx - 1 + tabs.length) % tabs.length;
        else if (e.key === 'Home') nextIdx = 0;
        else if (e.key === 'End') nextIdx = tabs.length - 1;
        if (nextIdx >= 0) {
          e.preventDefault();
          selectTab(tabs[nextIdx], true);
        }
      });

      const panel = panels[idx];
      if (panel) {
        panel.addEventListener('beforematch', () => selectTab(tab, false));
      }
    });

    // Auto-select visitor's platform cleanly without shifting layout
    const ua = (navigator.userAgentData?.platform || navigator.platform || navigator.userAgent || '').toLowerCase();
    if (ua.includes('mac')) {
      const macTab = document.getElementById('tab-mac');
      if (macTab) selectTab(macTab, false);
    } else if (ua.includes('linux')) {
      const cargoTab = document.getElementById('tab-cargo');
      if (cargoTab) selectTab(cargoTab, false);
    }
  }

  // --- Copy Buttons ---
  const statusRegion = document.getElementById('copy-status');
  function copyWithFeedback(btn, text) {
    if (!navigator.clipboard) return;
    navigator.clipboard.writeText(text).then(() => {
      const prev = btn.textContent;
      btn.textContent = 'Copied';
      if (statusRegion) statusRegion.textContent = 'Copied to clipboard.';
      setTimeout(() => {
        btn.textContent = prev;
      }, 1500);
    }).catch(() => {});
  }

  document.querySelectorAll('[data-copy]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const targetId = btn.getAttribute('data-copy');
      const target = targetId ? document.getElementById(targetId) : null;
      if (target) copyWithFeedback(btn, target.textContent.trim());
    });
  });

  // --- 3. Hero Specimen Stage & 12 FPS Biome Weather Canvas ---
  const SCENES = {
    campfire: {
      title: 'Warming hands in mountains',
      cmdBase: 'clairlune-cli generate --seed 7 --archetype mountains --activity warm_hands --palette classic --theme dark',
      weather: 'embers',
      clock: '22:14',
    },
    lanterns: {
      title: 'Guitar by lantern lake',
      cmdBase: 'clairlune-cli generate --seed 42 --archetype lantern_lake --palette kanagawa --theme dark',
      weather: 'lanterns',
      clock: '20:45',
    },
    fjord: {
      title: 'Fjord cascade at noon',
      cmdBase: 'clairlune-cli generate --seed 19 --archetype fjord --palette nord --theme light',
      weather: 'spindrift',
      clock: '12:30',
    },
    observatory: {
      title: 'Observatory in braille',
      cmdBase: 'clairlune-cli generate --seed 108 --archetype observatory --activity stargaze --palette dracula --letters braille --theme dark',
      weather: 'meteors',
      clock: '01:18',
    },
    shrine: {
      title: 'Hillside shrine in katakana',
      cmdBase: 'clairlune-cli generate --seed 64 --archetype shrine --palette everforest --letters kana --theme dark',
      weather: 'wisps',
      clock: '19:50',
    },
    lighthouse: {
      title: 'Lighthouse in dithered spray',
      cmdBase: 'clairlune-cli generate --seed 31 --archetype lighthouse --look dither --palette gruvbox --theme light',
      weather: 'spray',
      clock: '15:10',
    },
  };

  const heroImg = document.getElementById('stage-image');
  const heroCanvas = document.getElementById('stage-canvas');
  const clawdToggle = document.getElementById('clawd-toggle');
  const liveBtn = document.getElementById('live-toggle');
  const seedCmdEl = document.getElementById('stage-cmd');
  const stageLabelEl = document.getElementById('menubar-scene');
  const stageClockEl = document.getElementById('menubar-clock');
  const sceneChips = Array.from(document.querySelectorAll('.scene-chip'));

  let activeScene = 'campfire';
  let showBuddy = true;
  const reducedMotionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
  let isLive = !reducedMotionQuery.matches;
  let stageInView = true;

  function syncStage() {
    const spec = SCENES[activeScene];
    if (!spec || !heroImg) return;
    const suffix = showBuddy ? '' : '-solo';
    heroImg.src = `../assets/specimens/${activeScene}${suffix}.png`;
    heroImg.alt = `${spec.title}${showBuddy ? ' with pixel-art Clawd' : ' without Clawd'}`;
    if (stageLabelEl) stageLabelEl.textContent = `${spec.title} · ${ isLive ? 'LIVE' : 'STILL' }`;
    if (stageClockEl) stageClockEl.textContent = spec.clock;
    if (seedCmdEl) {
      seedCmdEl.textContent = showBuddy ? spec.cmdBase : `${spec.cmdBase} --no-buddy`;
    }
    if (liveBtn) {
      liveBtn.textContent = isLive ? 'Pause live' : 'Play live';
    }
  }

  sceneChips.forEach((chip) => {
    chip.addEventListener('click', () => {
      const id = chip.getAttribute('data-scene');
      if (!id || !SCENES[id]) return;
      activeScene = id;
      sceneChips.forEach((c) => c.setAttribute('aria-pressed', String(c === chip)));
      initParticles();
      syncStage();
    });
  });

  if (clawdToggle) {
    clawdToggle.addEventListener('change', () => {
      showBuddy = clawdToggle.checked;
      syncStage();
    });
  }

  if (liveBtn) {
    liveBtn.addEventListener('click', () => {
      isLive = !isLive;
      syncStage();
      if (!isLive && heroCanvas) {
        const ctx = heroCanvas.getContext('2d');
        if (ctx) ctx.clearRect(0, 0, heroCanvas.width, heroCanvas.height);
      }
    });
  }

  reducedMotionQuery.addEventListener('change', (e) => {
    if (e.matches) {
      isLive = false;
      syncStage();
    }
  });

  // Deterministic LCG for gentle 12 fps particles
  let particles = [];
  function initParticles() {
    particles = [];
    let s = 918273;
    const rand = () => {
      s = (s * 1664525 + 1013904223) >>> 0;
      return s / 4294967296;
    };
    for (let i = 0; i < 26; i++) {
      particles.push({
        x: rand(),
        y: rand(),
        speed: 0.004 + rand() * 0.01,
        drift: (rand() - 0.5) * 0.006,
        phase: rand() * Math.PI * 2,
        glyph: ['*', '·', '+', '°', '\''][i % 5],
      });
    }
  }
  initParticles();
  syncStage();

  if (heroCanvas) {
    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) stageInView = entry.isIntersecting;
      },
      { threshold: 0.05 }
    );
    io.observe(heroCanvas);

    const ctx = heroCanvas.getContext('2d');
    let lastTick = 0;
    const FRAME_MS = 1000 / 12; // Clairlune's default 12 fps cadence

    const tick = (now) => {
      requestAnimationFrame(tick);
      if (!isLive || !stageInView || !ctx) return;
      if (now - lastTick < FRAME_MS) return;
      lastTick = now;

      const w = heroCanvas.width;
      const h = heroCanvas.height;
      ctx.clearRect(0, 0, w, h);
      ctx.font = '11px "JetBrains Mono", monospace';

      const kind = SCENES[activeScene]?.weather || 'embers';
      for (let i = 0; i < particles.length; i++) {
        const p = particles[i];
        if (kind === 'embers') {
          // Rise gently from campfire zone
          const px = (0.47 + (p.x - 0.5) * 0.16 + Math.sin(now * 0.002 + p.phase) * 0.015) * w;
          p.y -= p.speed * 1.1;
          if (p.y < 0.22) p.y = 0.62;
          const py = p.y * h;
          const alpha = Math.max(0, Math.min(0.85, (p.y - 0.22) * 2.2));
          ctx.fillStyle = i % 2 === 0 ? `rgba(240, 162, 123, ${alpha})` : `rgba(255, 205, 130, ${alpha})`;
          ctx.fillText(i % 3 === 0 ? '*' : '·', px, py);
        } else if (kind === 'lanterns') {
          p.y -= p.speed * 0.35;
          p.x += Math.sin(now * 0.001 + p.phase) * 0.0008;
          if (p.y < 0.06) p.y = 0.58;
          ctx.fillStyle = 'rgba(242, 185, 118, 0.72)';
          ctx.fillText('¤', ((p.x % 1) + 1) % 1 * w, p.y * h);
        } else if (kind === 'spindrift' || kind === 'spray') {
          p.x = (p.x + p.speed * 1.4) % 1;
          p.y = (p.y + Math.sin(p.phase + now * 0.003) * 0.002 + 1) % 1;
          ctx.fillStyle = kind === 'spindrift' ? 'rgba(90, 118, 142, 0.55)' : 'rgba(130, 115, 92, 0.55)';
          ctx.fillText(i % 2 === 0 ? '·' : '~', p.x * w, (0.25 + p.y * 0.55) * h);
        } else if (kind === 'meteors') {
          p.x = (p.x + p.speed * 0.6) % 1;
          const twinkle = 0.25 + 0.55 * Math.abs(Math.sin(now * 0.002 + p.phase));
          ctx.fillStyle = `rgba(214, 186, 255, ${twinkle})`;
          ctx.fillText('⠁', p.x * w, (p.y * 0.55) * h);
        } else {
          // Shrine wisps
          const wx = (p.x + Math.sin(now * 0.0014 + p.phase) * 0.03) * w;
          const wy = (0.25 + p.y * 0.55 + Math.cos(now * 0.0012 + p.phase) * 0.02) * h;
          const glow = 0.3 + 0.5 * Math.abs(Math.sin(now * 0.0025 + p.phase));
          ctx.fillStyle = `rgba(167, 192, 128, ${glow})`;
          ctx.fillText('·', wx, wy);
        }
      }
    };
    requestAnimationFrame(tick);
  }

  // --- 4. Docs pages: contents folded on narrow screens, current section marked ---
  const tocDetails = document.querySelector('.doc-toc details');
  if (tocDetails) {
    if (window.matchMedia('(max-width: 1024px)').matches) tocDetails.open = false;
    const tocLinks = new Map(
      Array.from(tocDetails.querySelectorAll('a[href^="#"]')).map((a) => [decodeURIComponent(a.hash.slice(1)), a])
    );
    const heads = Array.from(document.querySelectorAll('.doc-prose h2[id]')).filter((h) => tocLinks.has(h.id));
    if (heads.length) {
      let current = null;
      let queued = false;
      const update = () => {
        queued = false;
        // The section whose heading last passed a line a third of the way down.
        const line = window.innerHeight / 3;
        let pick = heads[0];
        for (const h of heads) {
          if (h.getBoundingClientRect().top <= line) pick = h;
          else break;
        }
        if (pick.id === current) return;
        tocLinks.get(current)?.classList.remove('is-current');
        tocLinks.get(pick.id)?.classList.add('is-current');
        current = pick.id;
      };
      const queue = () => {
        if (!queued) {
          queued = true;
          requestAnimationFrame(update);
        }
      };
      window.addEventListener('scroll', queue, { passive: true });
      window.addEventListener('resize', queue, { passive: true });
      update();
    }
  }

  // --- 5. Evergreen Latest-Release Sync (low priority, graceful fallback) ---
  fetch('https://api.github.com/repos/yam333/Clairlune/releases/latest', { priority: 'low' })
    .then((r) => (r.ok ? r.json() : null))
    .then((rel) => {
      if (!rel || !rel.tag_name) return;
      const vTag = document.getElementById('version-badge');
      if (vTag) vTag.textContent = rel.tag_name;
      if (Array.isArray(rel.assets)) {
        const setup = rel.assets.find((a) => typeof a.name === 'string' && a.name.endsWith('x64-setup.exe'));
        const winBtn = document.getElementById('win-setup-link');
        if (setup?.browser_download_url && winBtn) {
          winBtn.href = setup.browser_download_url;
        }
      }
    })
    .catch(() => {});
})();
