// Clairlune static landing page interactivity:
// 1. Two-state theme comfort override (`dark-mode-toggle` pattern)
// 2. Accessible OS install tabs (`tabs` pattern + auto-OS initial selection)
// 3. Genuine Clairlune engine specimen stage + 12 fps biome particle canvas
// 4. Local in-browser Image-to-ASCII workbench (3 procedural scenes + drag/file)

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
    heroImg.src = `assets/specimens/${activeScene}${suffix}.png`;
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

  // --- 4. Local In-Browser Image-to-ASCII Workbench ---
  const asciiOut = document.getElementById('ascii-output');
  const asciiMeta = document.getElementById('ascii-dims');
  const rampSelect = document.getElementById('ascii-ramp');
  const widthRange = document.getElementById('ascii-width');
  const inspireBtn = document.getElementById('ascii-inspire');
  const fileInput = document.getElementById('ascii-file');
  const asciiStage = document.getElementById('ascii-dropzone');

  const RAMPS = {
    standard: ' .:-=+*#%@',
    detailed: ' .\':,"!;~+-x*oO0#%@',
    blocks: ' ░▒▓█',
    shapes: ' ·•◦○◎●■',
  };

  let inspireIndex = 0;
  let customImage = null;
  let customLabel = 'Crescent & peaks';
  const offCanvas = document.createElement('canvas');
  const offCtx = offCanvas.getContext('2d', { willReadFrequently: true });

  function drawProceduralSample(index, w, h) {
    offCanvas.width = w;
    offCanvas.height = h;
    if (!offCtx) return;
    offCtx.fillStyle = '#090B10';
    offCtx.fillRect(0, 0, w, h);

    if (index === 0) {
      customLabel = 'Crescent & peaks';
      // Glowing moon + subtracted shadow + sharp mountain ridgelines
      const mx = w * 0.62;
      const my = h * 0.36;
      const mr = h * 0.27;
      const grad = offCtx.createRadialGradient(mx, my, mr * 0.2, mx, my, mr * 2.1);
      grad.addColorStop(0, '#F5EFE2');
      grad.addColorStop(0.5, '#5C584F');
      grad.addColorStop(1, '#090B10');
      offCtx.fillStyle = grad;
      offCtx.fillRect(0, 0, w, h);

      offCtx.fillStyle = '#FAF5E8';
      offCtx.beginPath();
      offCtx.arc(mx, my, mr, 0, Math.PI * 2);
      offCtx.fill();

      offCtx.fillStyle = '#141720';
      offCtx.beginPath();
      offCtx.arc(mx - mr * 0.42, my - mr * 0.14, mr * 0.88, 0, Math.PI * 2);
      offCtx.fill();

      // Ridgelines
      offCtx.fillStyle = '#303440';
      offCtx.beginPath();
      offCtx.moveTo(0, h);
      offCtx.lineTo(w * 0.24, h * 0.44);
      offCtx.lineTo(w * 0.48, h * 0.72);
      offCtx.lineTo(w * 0.76, h * 0.49);
      offCtx.lineTo(w, h * 0.76);
      offCtx.lineTo(w, h);
      offCtx.fill();

      offCtx.fillStyle = '#161820';
      offCtx.beginPath();
      offCtx.moveTo(0, h);
      offCtx.lineTo(w * 0.14, h * 0.68);
      offCtx.lineTo(w * 0.41, h * 0.55);
      offCtx.lineTo(w * 0.69, h * 0.82);
      offCtx.lineTo(w * 0.88, h * 0.64);
      offCtx.lineTo(w, h);
      offCtx.fill();
    } else if (index === 1) {
      customLabel = 'Ringed planet';
      const cx = w * 0.5;
      const cy = h * 0.5;
      offCtx.save();
      offCtx.translate(cx, cy);
      offCtx.rotate(-0.32);
      // Outer ring
      offCtx.strokeStyle = '#E8E2D5';
      offCtx.lineWidth = h * 0.065;
      offCtx.beginPath();
      offCtx.ellipse(0, 0, w * 0.4, h * 0.13, 0, 0, Math.PI * 2);
      offCtx.stroke();
      // Inner ring
      offCtx.strokeStyle = '#9E988C';
      offCtx.lineWidth = h * 0.035;
      offCtx.beginPath();
      offCtx.ellipse(0, 0, w * 0.29, h * 0.09, 0, 0, Math.PI * 2);
      offCtx.stroke();
      offCtx.restore();

      // Planet globe
      const pr = h * 0.23;
      const pg = offCtx.createLinearGradient(cx - pr, cy - pr, cx + pr, cy + pr);
      pg.addColorStop(0, '#FFF8EB');
      pg.addColorStop(0.65, '#9A8C78');
      pg.addColorStop(1, '#1B1D24');
      offCtx.fillStyle = pg;
      offCtx.beginPath();
      offCtx.arc(cx, cy, pr, 0, Math.PI * 2);
      offCtx.fill();
    } else {
      customLabel = 'Torii & harvest moon';
      // Moon
      offCtx.fillStyle = '#F7F0E0';
      offCtx.beginPath();
      offCtx.arc(w * 0.5, h * 0.42, h * 0.3, 0, Math.PI * 2);
      offCtx.fill();
      // Water reflection
      offCtx.fillStyle = '#2A2E39';
      offCtx.fillRect(0, h * 0.72, w, h * 0.28);
      offCtx.fillStyle = '#BDB5A4';
      for (let ry = 0; ry < 7; ry++) {
        const rw = (7 - ry) * (w * 0.045);
        offCtx.fillRect(w * 0.5 - rw * 0.5, h * 0.75 + ry * (h * 0.032), rw, 2);
      }
      // Torii silhouette
      offCtx.fillStyle = '#090B10';
      offCtx.fillRect(w * 0.31, h * 0.28, w * 0.38, h * 0.048);
      offCtx.fillRect(w * 0.35, h * 0.37, w * 0.30, h * 0.032);
      offCtx.fillRect(w * 0.38, h * 0.31, w * 0.038, h * 0.43);
      offCtx.fillRect(w * 0.582, h * 0.31, w * 0.038, h * 0.43);
    }
  }

  function renderAsciiWorkbench() {
    if (!asciiOut || !offCtx) return;
    const cols = Number(widthRange?.value || 96);
    const mode = rampSelect?.value || 'standard';

    if (mode === 'braille') {
      const subW = cols * 2;
      const aspect = customImage ? customImage.height / customImage.width : 0.62;
      const rows = Math.max(14, Math.round(cols * aspect * 0.48));
      const subH = rows * 4;
      if (customImage) {
        offCanvas.width = subW;
        offCanvas.height = subH;
        offCtx.fillStyle = '#090B10';
        offCtx.fillRect(0, 0, subW, subH);
        offCtx.drawImage(customImage, 0, 0, subW, subH);
      } else {
        drawProceduralSample(inspireIndex, subW, subH);
      }
      const data = offCtx.getImageData(0, 0, subW, subH).data;
      const dots = [
        [0, 0, 0x01], [0, 1, 0x02], [0, 2, 0x04], [0, 3, 0x40],
        [1, 0, 0x08], [1, 1, 0x10], [1, 2, 0x20], [1, 3, 0x80],
      ];
      const lines = [];
      for (let r = 0; r < rows; r++) {
        let line = '';
        for (let c = 0; c < cols; c++) {
          let mask = 0;
          for (const [dx, dy, bit] of dots) {
            const idx = ((r * 4 + dy) * subW + (c * 2 + dx)) * 4;
            const lum = 0.2126 * data[idx] + 0.7152 * data[idx + 1] + 0.0722 * data[idx + 2];
            if (lum > 78) mask |= bit;
          }
          line += String.fromCharCode(0x2800 + mask);
        }
        lines.push(line);
      }
      asciiOut.textContent = lines.join('\n');
      if (asciiMeta) asciiMeta.textContent = `${customLabel} · ${cols}×${rows} chars · braille`;
      return;
    }

    const ramp = RAMPS[mode] || RAMPS.standard;
    const aspect = customImage ? customImage.height / customImage.width : 0.62;
    const rows = Math.max(14, Math.round(cols * aspect * 0.48));
    if (customImage) {
      offCanvas.width = cols;
      offCanvas.height = rows;
      offCtx.fillStyle = '#090B10';
      offCtx.fillRect(0, 0, cols, rows);
      offCtx.drawImage(customImage, 0, 0, cols, rows);
    } else {
      drawProceduralSample(inspireIndex, cols, rows);
    }
    const data = offCtx.getImageData(0, 0, cols, rows).data;
    const lines = [];
    for (let y = 0; y < rows; y++) {
      let row = '';
      for (let x = 0; x < cols; x++) {
        const i = (y * cols + x) * 4;
        const lum = (0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2]) / 255;
        const idx = Math.min(ramp.length - 1, Math.max(0, Math.floor(lum * ramp.length)));
        row += ramp[idx];
      }
      lines.push(row);
    }
    asciiOut.textContent = lines.join('\n');
    if (asciiMeta) asciiMeta.textContent = `${customLabel} · ${cols}×${rows} chars`;
  }

  function loadUserFile(file) {
    if (!file || !file.type.startsWith('image/')) return;
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      customImage = img;
      customLabel = file.name.slice(0, 28);
      renderAsciiWorkbench();
      URL.revokeObjectURL(url);
    };
    img.src = url;
  }

  if (inspireBtn) {
    inspireBtn.addEventListener('click', () => {
      customImage = null;
      inspireIndex = (inspireIndex + 1) % 3;
      renderAsciiWorkbench();
    });
  }
  if (rampSelect) rampSelect.addEventListener('change', renderAsciiWorkbench);
  if (widthRange) widthRange.addEventListener('input', renderAsciiWorkbench);
  if (fileInput) {
    fileInput.addEventListener('change', () => {
      if (fileInput.files && fileInput.files[0]) loadUserFile(fileInput.files[0]);
    });
  }
  if (asciiStage) {
    asciiStage.addEventListener('dragover', (e) => e.preventDefault());
    asciiStage.addEventListener('drop', (e) => {
      e.preventDefault();
      if (e.dataTransfer?.files?.[0]) loadUserFile(e.dataTransfer.files[0]);
    });
  }

  if (widthRange && window.innerWidth <= 640) {
    widthRange.value = '60';
  }
  renderAsciiWorkbench();

  // --- Docs pages: contents folded on narrow screens, current section marked ---
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
