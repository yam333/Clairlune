// Clairlune static landing page interactivity:
// 1. Two-state theme comfort override (`dark-mode-toggle` pattern)
// 2. Accessible OS install tabs (`tabs` pattern + auto-OS initial selection)
// 3. (The homepage's live parts are modules in js/, drawn by the real engine.)
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

  // --- 3. Docs pages: contents folded on narrow screens, current section marked ---
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

  // --- 4. Evergreen Latest-Release Sync (low priority, graceful fallback) ---
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
