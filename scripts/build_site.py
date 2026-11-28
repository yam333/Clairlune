#!/usr/bin/env python3
"""Compile Clairlune's Markdown docs into static Moonglass HTML pages under site/.

Generates:
  - site/guide/index.html      (from docs/guide.md)
  - site/cli/index.html        (from docs/cli.md)
  - site/changelog/index.html  (from CHANGELOG.md)
  - site/sitemap.xml           (with <lastmod> dates for all canonical pages)

Pure Python stdlib so CI needs zero extra packages.
"""

from __future__ import annotations

import datetime as dt
import html
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "site"
BASE_URL = "https://yam333.github.io/Clairlune"
REPO_BLOB = "https://github.com/yam333/Clairlune/blob/main"


def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    return m.group(1) if m else ""


VERSION = workspace_version()


def git_lastmod(paths: list[Path]) -> str:
    rels = [str(p.relative_to(ROOT)).replace("\\", "/") for p in paths]
    try:
        out = subprocess.check_output(
            ["git", "log", "-1", "--format=%cs", "--", *rels],
            cwd=ROOT,
            text=True,
        ).strip()
        if re.match(r"^\d{4}-\d{2}-\d{2}$", out):
            return out
    except Exception:
        pass
    return dt.date.today().isoformat()


def slugify(text: str) -> str:
    clean = re.sub(r"`([^`]*)`", r"\1", text)
    clean = re.sub(r"\[([^\]]+)\]\([^)]+\)", r"\1", clean)
    clean = clean.lower()
    clean = re.sub(r"[^\w\s-]", "", clean)
    clean = re.sub(r"\s+", "-", clean.strip())
    return clean


def rewrite_href(href: str, source_rel: str) -> str:
    if href.startswith(("http://", "https://", "mailto:", "#")):
        return href
    # Map internal doc links onto our rendered site routes
    cleaned = href.lstrip("./")
    if href.startswith("../"):
        target = cleaned[3:] if cleaned.startswith("../") else href.replace("../", "")
    elif source_rel.startswith("docs/"):
        target = f"docs/{cleaned}" if not cleaned.startswith("docs/") else cleaned
    else:
        target = cleaned

    frag = ""
    if "#" in target:
        target, frag = target.split("#", 1)
        frag = f"#{frag}"

    route_map = {
        "docs/guide.md": f"{BASE_URL}/guide/{frag}",
        "guide.md": f"{BASE_URL}/guide/{frag}",
        "docs/cli.md": f"{BASE_URL}/cli/{frag}",
        "cli.md": f"{BASE_URL}/cli/{frag}",
        "CHANGELOG.md": f"{BASE_URL}/changelog/{frag}",
        "README.md": f"{BASE_URL}/{frag}",
    }
    if target in route_map:
        return route_map[target]
    return f"{REPO_BLOB}/{target}{frag}"


def render_inline(text: str, refs: dict[str, str], source_rel: str) -> str:
    # Tokenize inline code spans first so inner characters are never parsed as markdown
    tokens: list[str] = []

    def stash_code(m: re.Match[str]) -> str:
        idx = len(tokens)
        tokens.append(f"<code>{html.escape(m.group(1))}</code>")
        return f"\x00CODE{idx}\x00"

    text = re.sub(r"`([^`]+)`", stash_code, text)
    text = html.escape(text, quote=False)

    # Standard links [label](url)
    def repl_link(m: re.Match[str]) -> str:
        label, url = m.group(1), html.unescape(m.group(2).strip())
        resolved = html.escape(rewrite_href(url, source_rel), quote=True)
        return f'<a href="{resolved}">{label}</a>'

    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", repl_link, text)

    # Reference links [label][ref] or [label]
    def repl_ref(m: re.Match[str]) -> str:
        label = m.group(1)
        key = (m.group(2) or label).lower()
        if key not in refs:
            return m.group(0)
        resolved = html.escape(rewrite_href(refs[key], source_rel), quote=True)
        return f'<a href="{resolved}">{label}</a>'

    text = re.sub(r"\[([^\]]+)\](?:\[([^\]]*)\])?", repl_ref, text)

    # Bold & italic
    text = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", text)

    for idx, code_html in enumerate(tokens):
        text = text.replace(f"\x00CODE{idx}\x00", code_html)
    return text


def split_table_row(line: str) -> list[str]:
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|"):
        s = s[:-1]
    # Split on unescaped pipes
    return [c.replace(r"\|", "|").strip() for c in re.split(r"(?<!\\)\|", s)]


def markdown_to_html(md: str, source_rel: str) -> tuple[str, list[tuple[str, str]]]:
    # Collect link reference definitions: [key]: url
    refs: dict[str, str] = {}
    body_lines: list[str] = []
    ref_re = re.compile(r"^\[([^\]]+)\]:\s*(\S+)\s*$")
    for raw in md.splitlines():
        m = ref_re.match(raw)
        if m:
            refs[m.group(1).lower()] = m.group(2)
        else:
            body_lines.append(raw)

    out: list[str] = []
    toc: list[tuple[str, str]] = []
    used_slugs: dict[str, int] = {}

    i = 0
    n = len(body_lines)
    seen_h1 = False

    while i < n:
        line = body_lines[i]
        stripped = line.strip()

        if not stripped:
            i += 1
            continue

        # Fenced code block
        if stripped.startswith("```"):
            i += 1
            code_lines: list[str] = []
            while i < n and not body_lines[i].strip().startswith("```"):
                code_lines.append(body_lines[i])
                i += 1
            if i < n:
                i += 1
            escaped = html.escape("\n".join(code_lines))
            out.append(
                f'<div class="terminal-card doc-code"><pre role="region" aria-label="Code example" tabindex="0"><code>{escaped}</code></pre></div>'
            )
            continue

        # ATX Headings
        hm = re.match(r"^(#{1,4})\s+(.+?)\s*$", line)
        if hm:
            level = len(hm.group(1))
            raw_title = hm.group(2)
            ver = re.match(r"^\[?(\d+\.\d+\.\d+)\]?", raw_title)
            base_slug = f"v{ver.group(1)}" if ver else (slugify(raw_title) or "section")
            count = used_slugs.get(base_slug, 0)
            used_slugs[base_slug] = count + 1
            slug = base_slug if count == 0 else f"{base_slug}-{count}"
            rendered = render_inline(raw_title, refs, source_rel)

            if level == 1 and not seen_h1:
                seen_h1 = True
                out.append(f'<h1 id="main-heading" tabindex="-1" class="doc-h1">{rendered}</h1>')
            else:
                tag_lvl = max(2, level)
                if tag_lvl == 2 and raw_title.strip().lower() != "unreleased":
                    plain_toc = re.sub(r"<[^>]+>", "", rendered).replace(" - ", " · ")
                    toc.append((slug, plain_toc))
                out.append(f'<h{tag_lvl} id="{slug}">{rendered}</h{tag_lvl}>')
            i += 1
            continue

        # GFM Table
        if "|" in line and i + 1 < n and re.match(r"^\s*\|?[\s:-]+\|[\s|:-]*$", body_lines[i + 1]):
            headers = split_table_row(line)
            i += 2
            rows: list[list[str]] = []
            while i < n and "|" in body_lines[i] and body_lines[i].strip():
                rows.append(split_table_row(body_lines[i]))
                i += 1
            th_html = "".join(
                f'<th scope="col">{render_inline(h, refs, source_rel) if h else "<span class=\"visually-hidden\">Item</span>"}</th>'
                for h in headers
            )
            tbody_rows = []
            for r in rows:
                padded = r + [""] * (len(headers) - len(r))
                # Phones stack each row; with 3+ columns the cells carry their header.
                labels = [re.sub(r"<[^>]+>", "", render_inline(h, refs, source_rel)) for h in headers]
                cells = "".join(
                    (
                        f'<td data-label="{html.escape(labels[ci], quote=True)}">'
                        if len(headers) > 2 and ci > 0 and labels[ci]
                        else "<td>"
                    )
                    + f"{render_inline(c, refs, source_rel)}</td>"
                    for ci, c in enumerate(padded[: len(headers)])
                )
                tbody_rows.append(f"<tr>{cells}</tr>")
            out.append(
                '<div class="table-scroll doc-table-wrap" role="region" aria-label="Reference table" tabindex="0">'
                f'<table class="crate-table doc-table"><thead><tr>{th_html}</tr></thead>'
                f'<tbody>{"".join(tbody_rows)}</tbody></table></div>'
            )
            continue

        # Unordered or Ordered List (supports wrapped continuation lines and simple 2-space nested bullets)
        lm = re.match(r"^(\s*)([-*]|\d+\.)\s+(.*)$", line)
        if lm:
            ordered = lm.group(2).endswith(".")
            tag = "ol" if ordered else "ul"
            items: list[tuple[str, list[str]]] = []
            while i < n:
                cur = body_lines[i]
                if not cur.strip():
                    # Look ahead: if next non-empty line is indented continuation or another list item of same type, keep going
                    if i + 1 < n and re.match(r"^(\s{2,}\S|[-*]|\d+\.\s)", body_lines[i + 1]):
                        i += 1
                        continue
                    break
                top_m = re.match(r"^([-*]|\d+\.)\s+(.*)$", cur)
                sub_m = re.match(r"^\s{2,}[-*]\s+(.*)$", cur)
                if top_m and (top_m.group(1).endswith(".") == ordered):
                    items.append((top_m.group(2).strip(), []))
                    i += 1
                elif sub_m and items:
                    items[-1][1].append(sub_m.group(1).strip())
                    i += 1
                elif cur.startswith("  ") and items:
                    if items[-1][1]:
                        items[-1][1][-1] += " " + cur.strip()
                    else:
                        items[-1] = (items[-1][0] + " " + cur.strip(), items[-1][1])
                    i += 1
                else:
                    break

            li_chunks = []
            for main_txt, subs in items:
                rendered_main = render_inline(main_txt, refs, source_rel)
                if subs:
                    sub_lis = "".join(f"<li>{render_inline(s, refs, source_rel)}</li>" for s in subs)
                    li_chunks.append(f"<li>{rendered_main}<ul>{sub_lis}</ul></li>")
                else:
                    li_chunks.append(f"<li>{rendered_main}</li>")
            out.append(f"<{tag}>{''.join(li_chunks)}</{tag}>")
            continue

        # Paragraph
        para: list[str] = []
        while (
            i < n
            and body_lines[i].strip()
            and not body_lines[i].strip().startswith("```")
            and not re.match(r"^#{1,4}\s+", body_lines[i])
            and not re.match(r"^([-*]|\d+\.)\s+", body_lines[i])
            and not (
                "|" in body_lines[i]
                and i + 1 < n
                and re.match(r"^\s*\|?[\s:-]+\|[\s|:-]*$", body_lines[i + 1])
            )
        ):
            para.append(body_lines[i].strip())
            i += 1
        joined = " ".join(para)
        if joined:
            out.append(f"<p>{render_inline(joined, refs, source_rel)}</p>")

    # An empty "## [Unreleased]" (or any heading with nothing under it before the
    # next heading of the same or higher level) says nothing on a page: drop it.
    kept: list[str] = []
    for idx, chunk in enumerate(out):
        head = re.match(r'^<h([2-4]) id="([^"]+)"', chunk)
        if head:
            nxt = out[idx + 1] if idx + 1 < len(out) else ""
            nxt_head = re.match(r"^<h([2-4]) ", nxt)
            if not nxt or (nxt_head and int(nxt_head.group(1)) <= int(head.group(1))):
                toc = [t for t in toc if t[0] != head.group(2)]
                continue
        kept.append(chunk)
    return "\n".join(kept), toc


def page_template(
    *,
    slug: str,
    nav_key: str,
    title: str,
    description: str,
    kicker: str,
    crumb_name: str,
    schema_type: str,
    body_html: str,
    toc: list[tuple[str, str]],
    source_rel: str,
    modified: str,
) -> str:
    canonical = f"{BASE_URL}/{slug}/"
    schema = {
        "@context": "https://schema.org",
        "@graph": [
            {
                "@type": schema_type,
                "@id": f"{canonical}#doc",
                "headline": title,
                "name": title,
                "description": description,
                "url": canonical,
                "isPartOf": {"@id": f"{BASE_URL}/#website"},
                "about": {"@id": f"{BASE_URL}/#app"},
                "author": {"@type": "Person", "name": "yam333", "url": "https://github.com/yam333"},
                "dateModified": modified,
                "inLanguage": "en",
            },
            {
                "@type": "BreadcrumbList",
                "itemListElement": [
                    {"@type": "ListItem", "position": 1, "name": "Clairlune", "item": f"{BASE_URL}/"},
                    {"@type": "ListItem", "position": 2, "name": crumb_name, "item": canonical},
                ],
            },
        ],
    }

    def nav_attr(key: str) -> str:
        return ' aria-current="page"' if key == nav_key else ""

    toc_items = "".join(
        f'<li><a href="#{html.escape(anchor)}">{html.escape(label)}</a></li>'
        for anchor, label in toc
    )
    toc_aside = (
        '<nav class="doc-toc" aria-label="On this page"><details open>'
        '<summary class="sub-kicker">On this page</summary>'
        f'<ol role="list">{toc_items}</ol></details></nav>'
        if toc_items
        else ""
    )
    source_link = (
        f'<p class="doc-source">From <a href="{REPO_BLOB}/{source_rel}">{source_rel}</a>'
        f' on GitHub, built with every change.</p>'
    )

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta name="color-scheme" content="light dark">
  <script>try{{document.querySelector('meta[name="color-scheme"]').content=localStorage.getItem("color-scheme")??"light dark"}}catch{{}}</script>

  <title>{html.escape(title)}</title>
  <meta name="description" content="{html.escape(description, quote=True)}">
  <link rel="canonical" href="{canonical}">

  <meta property="og:type" content="article">
  <meta property="og:site_name" content="Clairlune">
  <meta property="og:title" content="{html.escape(title, quote=True)}">
  <meta property="og:description" content="{html.escape(description, quote=True)}">
  <meta property="og:url" content="{canonical}">
  <meta property="og:image" content="{BASE_URL}/assets/social-preview.png">
  <meta name="twitter:card" content="summary_large_image">

  <link rel="icon" href="../assets/favicon.svg" type="image/svg+xml">
  <link rel="apple-touch-icon" href="../assets/icon-128.png">
  <link rel="stylesheet" href="../styles.css">

  <script type="application/ld+json">
{json.dumps(schema, indent=2)}
  </script>
</head>
<body>
  <a href="#main-heading" class="skip-link visually-hidden">Skip to main content</a>

  <header class="site-header">
    <div class="header-inner">
      <div class="header-actions">
        <a href="../" class="brand-lockup">
          <svg class="brand-mark" aria-hidden="true" viewBox="0 0 32 32">
            <circle class="moon" cx="14" cy="18" r="12"/>
            <rect class="screen" x="14" y="2" width="16" height="16" rx="2"/>
          </svg>
          <span class="wordmark">Clair<em>lune</em></span>
        </a>
        <span class="version-tag" id="version-badge">v{VERSION}</span>
      </div>

      <nav class="primary-nav" aria-label="Primary">
        <ul>
          <li><a href="../"{nav_attr("home")}>Overview</a></li>
          <li><a href="../guide/"{nav_attr("guide")}>Guide</a></li>
          <li><a href="../cli/"{nav_attr("cli")}>CLI</a></li>
          <li><a href="../changelog/"{nav_attr("changelog")}>Changelog</a></li>
          <li><a href="https://github.com/yam333/Clairlune">GitHub</a></li>
        </ul>
      </nav>

      <div class="header-actions">
        <button type="button" id="theme-toggle" class="btn-quiet theme-btn">
          <span class="to-dark">
            <svg aria-hidden="true" width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4">
              <path d="M13.2 9.8A5.5 5.5 0 0 1 6.2 2.8a5.5 5.5 0 1 0 7 7Z"/>
            </svg>
            Night
            <span class="visually-hidden">theme</span>
          </span>
          <span class="to-light">
            <svg aria-hidden="true" width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4">
              <circle cx="8" cy="8" r="3.2"/>
              <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4"/>
            </svg>
            Day
            <span class="visually-hidden">theme</span>
          </span>
        </button>
        <a href="https://github.com/yam333/Clairlune/releases/latest" class="btn-primary">Download</a>
      </div>
    </div>
    <nav class="mobile-subnav" aria-label="Site pages">
      <ul>
        <li><a href="../"{nav_attr("home")}>Overview</a></li>
        <li><a href="../guide/"{nav_attr("guide")}>Guide</a></li>
        <li><a href="../cli/"{nav_attr("cli")}>CLI</a></li>
        <li><a href="../changelog/"{nav_attr("changelog")}>Changelog</a></li>
        <li><a href="https://github.com/yam333/Clairlune">GitHub</a></li>
      </ul>
    </nav>
  </header>

  <main id="main-content" class="frame">
    <div class="section-kicker">
      <span>{html.escape(kicker)}</span>
      <span><a href="../">Clairlune</a> / {html.escape(crumb_name)}</span>
    </div>

    <div class="doc-layout band">
      {toc_aside}
      <article class="doc-prose">
{body_html}
        {source_link}
      </article>
    </div>
  </main>

  <footer class="site-footer">
    <div class="maker-block">
      <span class="sub-kicker">Made by</span>
      <a href="https://github.com/yam333" class="signature-link" aria-label="yam333 on GitHub">
        <span class="signature-text">yam333</span>
        <svg class="signature-stroke" aria-hidden="true" viewBox="0 0 120 14">
          <path d="M 2 9 Q 56 2 114 11"/>
          <circle class="ember-dot" cx="28" cy="6" r="1.6"/>
          <circle class="ember-dot" cx="58" cy="5" r="1.4"/>
          <circle class="ember-dot" cx="84" cy="7" r="1.7"/>
          <circle class="ember-dot" cx="102" cy="9" r="1.3"/>
        </svg>
      </a>
      <p>If Clairlune made your desk a little nicer, say hi.</p>
    </div>

    <div class="colophon">
      <ul class="footer-links" role="list">
        <li><a href="../">Overview</a></li>
        <li><a href="../guide/">User guide</a></li>
        <li><a href="../cli/">CLI docs</a></li>
        <li><a href="../changelog/">Changelog</a></li>
        <li><a href="https://github.com/yam333/Clairlune/releases/latest">Releases</a></li>
        <li><a href="https://crates.io/crates/clairlune">crates.io</a></li>
        <li><a href="https://github.com/yam333/Clairlune">Source on GitHub</a></li>
      </ul>
      <div class="colophon-legal">
        <p>Code dual-licensed under <a href="https://github.com/yam333/Clairlune/blob/main/LICENSE-MIT">MIT</a> or <a href="https://github.com/yam333/Clairlune/blob/main/LICENSE-APACHE">Apache-2.0</a>. Typefaces under the SIL Open Font License 1.1 (<a href="https://github.com/yam333/Clairlune/blob/main/NOTICE">NOTICE</a>).</p>
        <p>Unofficial fan project, not affiliated with or endorsed by Anthropic. Clawd is Anthropic's character.</p>
      </div>
    </div>
  </footer>

  <script src="../app.js" defer></script>
</body>
</html>
"""


def main() -> None:
    pages = [
        {
            "src": ROOT / "docs" / "guide.md",
            "source_rel": "docs/guide.md",
            "slug": "guide",
            "nav_key": "guide",
            "title": "Clairlune User Guide: Install, Live Wallpapers, Multiple Monitors & Timeline",
            "description": "Complete user guide for Clairlune on Windows 10/11 and macOS 13+: installation, live ASCII-art wallpapers, multi-monitor setup, timeline scheduling, and troubleshooting.",
            "kicker": "Documentation / User Guide",
            "crumb_name": "User Guide",
            "schema_type": "TechArticle",
            "priority": "0.9",
        },
        {
            "src": ROOT / "docs" / "cli.md",
            "source_rel": "docs/cli.md",
            "slug": "cli",
            "nav_key": "cli",
            "title": "clairlune-cli Reference: Headless ASCII-Art Wallpaper & GIF Generator",
            "description": "Command-line reference for clairlune-cli on Windows, macOS, and Linux: deterministic 4K PNG stills, looping GIFs, frozen scene packages, contact sheets, and desktop wallpaper control.",
            "kicker": "Documentation / Command-Line Tool",
            "crumb_name": "CLI Reference",
            "schema_type": "TechArticle",
            "priority": "0.8",
        },
        {
            "src": ROOT / "CHANGELOG.md",
            "source_rel": "CHANGELOG.md",
            "slug": "changelog",
            "nav_key": "changelog",
            "title": "Clairlune Changelog & Release Notes",
            "description": "Version-by-version release notes and changelog for Clairlune desktop and clairlune-cli.",
            "kicker": "Documentation / Release History",
            "crumb_name": "Changelog",
            "schema_type": "WebPage",
            "priority": "0.7",
        },
    ]

    # The homepage names the version twice (badge and schema): keep both current.
    index = SITE / "index.html"
    index_html = index.read_text(encoding="utf-8")
    stamped = re.sub(r'("softwareVersion": ")[^"]*(")', rf"\g<1>{VERSION}\g<2>", index_html)
    stamped = re.sub(r'(id="version-badge">v)[^<]*', rf"\g<1>{VERSION}", stamped)
    if stamped != index_html:
        index.write_text(stamped, encoding="utf-8", newline="\n")
        print(f"stamped site/index.html with {VERSION}")

    sitemap_entries = [
        (f"{BASE_URL}/", git_lastmod([SITE / "index.html", SITE / "styles.css"]), "weekly", "1.0")
    ]

    for p in pages:
        md_text = p["src"].read_text(encoding="utf-8")
        # Strip duplicate in-doc bullet TOC in docs/guide.md since our desktop layout generates a sticky TOC
        if p["slug"] == "guide":
            md_text = re.sub(
                r"\n- \[Install\]\(#install\)(?:\n- \[[^\]]+\]\(#[^)]+\))+",
                "",
                md_text,
                count=1,
            )
        body_html, toc = markdown_to_html(md_text, p["source_rel"])
        full_html = page_template(
            slug=p["slug"],
            nav_key=p["nav_key"],
            title=p["title"],
            description=p["description"],
            kicker=p["kicker"],
            crumb_name=p["crumb_name"],
            schema_type=p["schema_type"],
            body_html=body_html,
            toc=toc,
            source_rel=p["source_rel"],
            modified=git_lastmod([p["src"]]),
        )
        dest = SITE / p["slug"] / "index.html"
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(full_html, encoding="utf-8", newline="\n")
        print(f"wrote {dest.relative_to(ROOT)} ({len(full_html) // 1024} KB)")
        sitemap_entries.append((f"{BASE_URL}/{p['slug']}/", git_lastmod([p["src"]]), "weekly", p["priority"]))

    url_nodes = "\n".join(
        f"""  <url>
    <loc>{loc}</loc>
    <lastmod>{lastmod}</lastmod>
    <changefreq>{freq}</changefreq>
    <priority>{prio}</priority>
  </url>"""
        for loc, lastmod, freq, prio in sitemap_entries
    )
    sitemap_xml = f"""<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{url_nodes}
</urlset>
"""
    (SITE / "sitemap.xml").write_text(sitemap_xml, encoding="utf-8", newline="\n")
    print("wrote site/sitemap.xml")


if __name__ == "__main__":
    main()
