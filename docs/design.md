# Clairlune UI design

Research, decisions and wireframes for the Settings, Archive, Library and Timeline windows (PLAN.md section 10a). Written before any UI code; the tokens at the end are implemented as one Rust module (`crates/app/src/tokens.rs`), and UI code may use no other colours, sizes or durations.

The product in one line: the wallpaper is the hero, and the app around it is a quiet, precise instrument for choosing what the wallpaper does.

## 1. Sources and what we take from each

### Platform guidelines

**Microsoft Fluent 2 / Windows app design** ([layout](https://fluent2.microsoft.design/layout), [typography](https://learn.microsoft.com/en-us/windows/apps/design/style/typography), [style](https://learn.microsoft.com/en-us/windows/apps/design/style/))
- 4 px base unit; groups are made with space, not divider lines.
- One typeface through the app; hierarchy from a type ramp, not from colour.
- No all-caps for emphasis (hard to read).
- *Decision:* 4 px grid, whitespace before lines, sentence case everywhere. On Windows the Settings window follows the Windows 11 settings shape: a left navigation list, the page on the right, a page title at the top.

**Apple HIG, macOS settings and windows** ([settings](https://developer.apple.com/design/human-interface-guidelines/settings), [windows](https://developer.apple.com/design/human-interface-guidelines/windows))
- Settings open from the app menu with Cmd-, ; the window title follows the visible pane; the window reopens on the last pane viewed.
- Minimize and zoom are disabled for settings; switches carry a short description; consequential changes confirm.
- A stable layout: people learn where things are.
- *Decision:* Cmd-, (and Ctrl-, on Windows) opens Settings; the title reads "Clairlune Settings: Schedule"; the last section is restored; no minimize/maximize on Settings. Destructive actions (clear archive, restore original wallpaper) get an inline undo, not a modal.

**GNOME HIG** ([boxed lists](https://developer.gnome.org/hig/patterns/containers/boxed-lists.html))
- Preferences as boxed lists: rows that hold a label, a description and a control, grouped in sections with headings; link rows end in a go-next arrow.
- *Decision:* every setting is a row: name, one-line description under it, control on the right. Sections are titled groups of rows. This one pattern carries all of Settings, which keeps it calm and scannable on all three systems.

### Fundamentals

**Refactoring UI** (Wathan, Schoger)
- Hierarchy from weight and colour contrast before size; de-emphasise secondary content instead of shouting primary content.
- A constrained spacing and type scale chosen up front; no in-between values.
- Labels are often unnecessary when the value explains itself; avoid grey text on coloured backgrounds.
- *Decision:* two text colours per theme (primary, secondary) and one disabled tone. Section titles are the same size as row labels but heavier. Values like times and seeds are shown in mono without a label where obvious.

**Laws of UX** (lawsofux.com)
- Hick: fewer visible choices decide faster. Fitts: big, near targets. Jakob: users expect it to work like the other apps they use. Doherty: answer within 400 ms. Aesthetic-usability: polish buys patience, so it must not hide problems.
- *Decision:* ten sidebar sections, each short; advanced items folded under a "More" row rather than shown. Every control acknowledges input at once; generation shows progress in place if it takes over 400 ms. Platform-native conventions (Jakob) win over novelty.

**Nielsen Norman Group, 10 usability heuristics**
- Visibility of system status, match with the real world, user control (undo), consistency, error prevention, recognition over recall, flexibility, minimalism, error recovery, help.
- *Decision:* the current wallpaper and next change time are always visible at the top of Settings. Plain words ("Change wallpaper every", not "Rotation interval"). Undo for anything destructive. Errors say what happened and the next step.

**Butterick, Practical Typography**
- Body text 15-25 px on screen, line length 45-90 characters, line spacing 120-145 % of the size, real weights, no faux bold, sparing use of underline and all-caps, letterspace caps if used.
- *Decision:* 14 px body (desktop UI is denser than prose, and platform UIs sit at 13-15 px), 20 px line height (143 %), descriptions capped at ~70 characters per line by the page's max width.

**WCAG 2.2** ([what's new](https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/))
- AA: 4.5:1 text contrast (3:1 for large text and UI component boundaries), 2.4.11 focus not obscured, 2.5.7 dragging alternatives, 2.5.8 targets at least 24x24 px. AAA 2.4.13 focus appearance: indicator area at least a 2 px perimeter and 3:1 change.
- *Decision:* every text/background pair in the tokens is checked by a test (4.5:1, or 3:1 for borders and focus). Focus rings are 2 px in the accent, offset 2 px, which also satisfies the AAA focus appearance criterion. Controls are at least 28 px tall (PLAN.md) and the timeline offers keyboard and button alternatives to every drag.

### Reference products

**Linear** ([design refresh](https://linear.app/now/behind-the-latest-design-refresh), [redesign](https://linear.app/blog/how-we-redesigned-the-linear-ui))
- Why it works: dense information without noise. Elements don't compete for attention they haven't earned; the sidebar is dimmer than the content; near-native feel by leaning on platform conventions.
- *Take:* the sidebar sits one step dimmer than the page; only the selected section is emphasised.

**Things 3** (Cultured Code)
- Why it works: one accent colour on an off-white canvas, generous space, a natural hierarchy where each screen has one obvious subject. "Things uses silence."
- *Take:* one accent, used only for primary action, selection and the timeline's "now" marker. Each screen has one subject (the wallpaper preview or the list), everything else steps back.

**Raycast, Arc**
- Why they work: keyboard-first with visible shortcuts; restrained chrome that gets out of the way of content.
- *Take:* every window is fully keyboard-operable; shortcuts appear in menus, not as badges sprinkled over the UI.

**Teenage Engineering** ([design guide](https://blakecrosley.com/guides/design/teenage-engineering))
- Why it works: constraints as a visible design choice; a small, legible surface; a few colours that each mean something; playful only where it serves.
- *Take:* the pixel-art and ASCII language of the wallpapers is the app's one playful element, used in empty states and the preview, never as decoration elsewhere.

**Panic (Nova, Playdate)**
- Why it works: craft in small details (alignment, icon consistency, copy with personality but no fluff).
- *Take:* consistent 1.5 px icon stroke at one size, copy that is short and specific.

**Swiss / International Typographic Style; Dieter Rams**
- Why it works: grid, asymmetric left-aligned layouts, typography as structure, objective clarity; "less, but better", honest, unobtrusive, thorough down to the last detail.
- *Take:* a visible 8 px rhythm, left alignment, no centred card layouts; nothing on screen that carries no information.

## 2. What generic UI looks like (to avoid)

From current design critiques plus PLAN.md's list. The common thread: design with no author, made of defaults.

- Purple, violet or indigo gradients; cyan on dark; glowing accents; gradient text.
- Inter everywhere at default settings; random serif/sans mixing; italic for no reason.
- Centred hero, three rounded feature cards, card-in-card, identical card grids on every screen.
- Oversized corner radii, generic soft drop shadows, frosted-glass cards everywhere (one material on one surface is a decision; glass on everything is a default).
- Emoji as icons or decoration; decorative illustrations carrying no information.
- Modals as the default for everything; badges, pills and tooltips on everything.
- Marketing copy inside settings ("Unleash", "Seamlessly", exclamation marks).
- Stock framework styling (default egui look and font).
- Contrast that fails WCAG because the palette was picked by mood.

**The test** we apply in review: if it looks like a template anyone could have shipped, it fails.

## 3. Decisions

Revised in 0.22 ("Moonglass"): the same Settings window on Windows and macOS, built around one material, reeded glass, over the wallpaper itself.

- **Material.** Settings' sidebar is a pane of reeded (fluted) glass laid over the wallpaper on screen: 12 px flutes, each showing the scene under it 1.35 times wider, lit on its left edge and shaded on its right, under a tint that deepens towards the bottom. The page is smoked glass: a blurred copy of the wallpaper under a near-opaque pane, with fine grain. Without a wallpaper the same surfaces are flat. Glass appears there and nowhere else.
- **Window chrome.** Windows: no system title bar; the window draws its own minimize and close buttons and keeps the system shadow, rounded corners, dragging (top 36 px) and resizing (edges). macOS: a see-through title bar with the system's buttons over the sidebar.
- **Typefaces.** Instrument Serif (OFL) for page, window and dialog titles and the wordmark ("Clair" upright, "lune" italic): a condensed display serif that reads as drawn, not defaulted. IBM Plex Sans (OFL) for the UI. JetBrains Mono (OFL, already bundled for the wallpapers) for data and for the small spaced capitals of group labels.
- **Type scale (4 sizes).** 12 / 14 / 28 / 40 px with line heights 16 / 20 / 36 / 48 (group labels 11 px mono, spaced). Weights 400 and 600 (500 for row names). Capitals only for the mono group labels.
- **Colour.** Neutral surfaces from the wallpaper palettes (paper and cool ink-black), two text tones, hairlines, and one accent: ember, the colour of the campfire embers in the wallpapers, as a soft fill (`accent`), a deeper one for switches and selection dots (`accent_deep`) and a text-safe one for links and tags (`accent_text`). Full light and dark sets that follow the app theme.
- **Depth.** The glass, 1 px hairlines and highlights, translucent cards on the glass; one elevation level (popovers and the undo toast) with a single tight shadow.
- **Shape.** Radii 10 px for controls, 14 px for panels and cards. Pill shapes only for the switch and the filter chips. Icons: drawn in code on a 16 px grid, 1.5 px stroke.
- **Motion.** 120 / 160 / 200 ms, ease-out only; respects the system's reduce-motion setting by cutting to 0.
- **Layout.** Left-aligned on an 8 px rhythm; sidebar 240 px with the sections and, at its foot, what's on screen now (thumbnail, title, LIVE or STILL, seed, next change) and the New wallpaper button; each page opens with its serif title and a one-line mono summary. Landscapes are picked from tiles that show each one.
- **Copy.** Sentence case, short, specific. Descriptions say what happens, not how good it is. No exclamation marks.
- **Playfulness budget.** Pixel-art Clawd appears in empty states and the "generating" state only (fan art, so also only when Buddy is on); the moon mark in the title area. Nowhere else.

## 4. Low-fidelity wireframes

### Settings (all platforms; native window chrome)

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Clairlune Settings: Schedule                                    [_][x]   │
├────────────────┬─────────────────────────────────────────────────────────┤
│  Now           │  ┌─────────────────────────────────────┐                │
│  Rotation      │  │                                     │  stroll in cave│
│▌ Schedule      │  │        current wallpaper preview    │  seed 7  dark  │
│  Theme         │  │              (16:9, 10 px radius)   │  next 14:00    │
│  Display       │  └─────────────────────────────────────┘                │
│  Live          │  [ New wallpaper ]  Previous                            │
│  Buddy         │                                                         │
│  Content       │  Schedule                                               │
│  Archive       │  ─────────────────────────────────────────────────────  │
│  Export        │  Change wallpaper every                  [ 1 hour   v ] │
│  General       │  A new scene on this interval. Off: only on demand.     │
│                │                                                         │
│                │  Also change at login                            [ on ] │
│                │  Start each session with a fresh scene.                 │
│                │                                                         │
│  ☾ 0.7.0       │  Pause                                           [off ] │
│                │  Keep the current wallpaper until you resume.           │
└────────────────┴─────────────────────────────────────────────────────────┘
```

- The preview strip sits above every page, so status (heuristic 1) is always visible: what's on screen, its seed and theme, when it changes next.
- One row = name (14/600), description (14/400 secondary), control right-aligned on the row's first line.

### Content section (checkbox lists)

```
│  Landscapes                                                      All ▾ │
│  ───────────────────────────────────────────────────────────────────── │
│  [x] Canyon     [x] Mountains   [x] Forest     [x] Desert              │
│  [x] Snowfield  [x] Shore       [x] City       [x] Night sky  [x] Cave │
│                                                                         │
│  Activities                                                              │
│  [x] Warming hands  [x] Strolling  [x] Stargazing (night)  ...          │
```

Each checkbox label can show a 48 px pixel thumbnail on hover (keyboard focus too), so the names are recognised, not recalled.

Since 0.22 landscapes are tiles instead: each shows that landscape (drawn once per generator version and theme, in the background, at a wallpaper's scale and shrunk), so the names are recognised, not recalled. Weather and activities are chips.

### Archive window

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Clairlune Archive                     Keep 7 days · 214 MB of 1 GB       │
├──────────────────────────────────────────────────────────────────────────┤
│  Today                                                                    │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐                      │
│  │  thumb   │ │  thumb   │ │  thumb   │ │  thumb   │                      │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘                      │
│  14:00 cave   13:00 shore  12:00 forest 11:00 desert                      │
│  Yesterday                                                                │
│  ...                                                                      │
├──────────────────────────────────────────────────────────────────────────┤
│  Selected: 13:00 stroll on the shore, seed 7      Apply  Pin  Save  Delete│
└──────────────────────────────────────────────────────────────────────────┘
```

Thumbnails are the content; chrome is a single action bar for the selection (no per-thumbnail buttons, no card frames).

### Library and Timeline

Library follows the Archive grid without time grouping, plus user-given names. The Timeline is a 24-hour horizontal strip per day (or seven stacked for the weekly layout) with slots as blocks showing the wallpaper's colour band; the "now" marker is the only accent on the screen. Every drag has a keyboard and a button equivalent (WCAG 2.5.7).

## 5. Every state, designed

| State | Treatment |
|---|---|
| First run | Settings opens on "Now" with the first wallpaper being generated in the preview; one sentence explains the tray icon. |
| Generating | Preview dims to 60 %; a one-line status in mono ("rendering 2560x1440...") under it; buttons disabled, not hidden. |
| Live unavailable here | The Live option is shown disabled with the reason on the row ("GNOME doesn't allow apps behind the desktop icons; Still frame is used"). |
| Buddy off | Empty states use the moon mark instead of Clawd. |
| Empty archive / library / timeline day | Pixel-art Clawd (Buddy on) or moon, one sentence, one action. |
| Export in progress | Inline progress on the row that started it; result row with "Show in folder". |
| Error | Inline, under the control that failed: what happened, and the next step. Logs link under General. |
| Restore original wallpaper | Row in General with an undo toast for 8 s. |

## 6. Review checklist (also in the PR template once UI lands)

- [ ] Only token values: colours, spacing, radii, type sizes, durations.
- [ ] Only the four type sizes; weights 400/600 (500 for row names).
- [ ] One accent, only on primary action, selection, focus and "now".
- [ ] Glass only on Settings' sidebar and page; no gradients elsewhere, no glow, emoji, badges on everything, card-in-card, centred layouts.
- [ ] Every control has a one-line description; sentence case; no exclamation marks.
- [ ] Keyboard: every action reachable, focus ring visible, Esc closes menus and dialogs (never the window).
- [ ] Contrast test passes for every token pair used.
- [ ] Every state in section 5 handled for this screen.
- [ ] Screenshot in light and dark on each OS reviewed against section 2.

## 7. Tokens

Implemented in `crates/app/src/tokens.rs` (toolkit-independent constants; checked by tests for WCAG contrast).

| Token | Light | Dark |
|---|---|---|
| `bg` page | `#F6F3EC` | `#121419` |
| `bg_sidebar` | `#EFEBE2` | `#0F1116` |
| `bg_raised` (rows, inputs) | `#FCFBF8` | `#1B1D23` |
| `hairline` | `#DED8CC` | `#2A2C31` |
| `border_control` (3:1) | `#847E72` | `#7A766E` |
| `text` | `#1C1E24` | `#ECE6D8` |
| `text_secondary` | `#5D5A54` | `#A9A49A` |
| `text_disabled` | `#A29C90` | `#5F5B54` |
| `accent` (fills) | `#E8875E` | `#F0A27B` |
| `accent_deep` (switches, dots) | `#CF6640` | `#DD7D55` |
| `accent_text` (links, tags, focus) | `#A64D29` | `#F0A27B` |
| `on_accent` | `#1A1210` | `#1A1210` |
| `danger` | `#A5321F` | `#FF8A75` |

The glass has its own set (`Colors::glass`): the pane colour and opacity (at least 0.8, so text on it keeps the contrast measured against `bg`), the sidebar tint from top to bottom, the flutes' light and shade, the card fill and the grain's strength.

Spacing: 0, 4, 8, 12, 16, 24, 32, 48, 64 px. Radii: 10 (controls), 14 (panels). Type: 12/16, 14/20, 28/36, 40/48. Motion: 120, 160, 200 ms, ease-out `cubic-bezier(0.2, 0, 0, 1)`. Control height 32 px (36 px for primary buttons). Focus ring 2 px `accent_text`, 2 px offset.

## 8. Toolkit: egui (spike, 2026-11-20)

PLAN.md asked for an egui-vs-Slint spike against two bars: the design bar of section 10a, and the event-loop rule of PLAN.md section 3.4 (tray plus a window opened and closed three times on one loop).

**Slint is ruled out by its licence.** It is offered under GPL-3.0, a royalty-free licence with conditions, or a paid licence. None of them is permissive, and Clairlune ships under MIT OR Apache-2.0 with a permissive-only dependency policy (`deny.toml`).

**egui passes both bars.**
- *Licence:* MIT OR Apache-2.0 (the whole stack: egui, egui-winit, egui-wgpu, wgpu). Its text stack (skrifa, harfrust, vello_cpu) avoids the unmaintained ttf-parser.
- *Event loop:* egui-winit and egui-wgpu plug into our own winit 0.30 loop; no eframe. `clairlune --ui-test <dir>` scripts the pass criterion: Settings opened and closed three times beside the tray icon, a screenshot of each opening, then exit. First open 563 ms (GPU start-up), later opens about 290 ms. Every opening creates the window, GPU device and surface; closing drops them all, so nothing of the UI stays in memory while the app idles in the tray.
- *Design bar:* restyled entirely from the tokens: IBM Plex Sans and JetBrains Mono (egui's default fonts are not even compiled in), our colours, radii, spacing and type sizes for both themes, hairlines, no shadows except the one elevation level, a custom switch and segmented control. One trap found: egui keeps a style per theme and follows the window's theme, so the app pins egui's theme and styles that one, or egui's stock light look shows through.
- *Low power:* the settings window asks wgpu for the low-power (integrated) GPU.

Versions: egui 0.37 (Rust 1.95 for the app crate only; the libraries and CLI keep 1.90), wgpu 30 with the native backends only (DX12, Metal, Vulkan, GLES; no WebGPU or WebGL).

Screenshots from the spike: dark Schedule, light Theme, dark Archive pages. Open points for the Settings work (14d): the empty preview needs its designed empty state (moon or Clawd), the remaining sections, keyboard navigation through the sidebar, and a screenshot review pass on macOS and Linux.
