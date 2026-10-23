# Rabbit Design System — "Precision Media Workstation"

## 0. Principles

1. **Instrument, not dashboard.** Rabbit behaves like a calibrated tool: every number is real, every readout is
   aligned, nothing decorates. Borders and type do the work that shadows and gradients do elsewhere.
2. **One action per view.** Each view has exactly one amber primary action. Everything else is quieter.
3. **Data in mono, language in sans.** Sizes, speeds, ETAs, resolutions, codecs and timestamps are always set in
   the mono face with tabular figures, so columns never jitter while values change.
4. **Honest states.** Loading shows what is happening ("Resolving with yt-dlp · 1.4 s"), errors say what failed and
   what to do, unsupported means unsupported.
5. **Density with air.** 13 px base text, 32 px controls, 4 px grid — compact, but separated by consistent
   hairlines and generous *alignment* rather than generous whitespace.

Signature element: **the ruler.** Progress is drawn as a hairline track with tick marks every 10 % and an amber
fill; segmented transfers show one micro-bar per live segment underneath. It doubles as real diagnostic data.

## 1. Color

All colors are CSS custom properties in `src/design/tokens.css`; Tailwind reads them via `@theme`.

### Surfaces (graphite → charcoal)
| Token | Hex | Use |
|---|---|---|
| `--bg-0` | `#0D0E10` | Title bar, sidebar, deepest layer |
| `--bg-1` | `#131417` | Main content canvas |
| `--bg-2` | `#191B1F` | Panels, rows, cards |
| `--bg-3` | `#202328` | Inputs, hover fill |
| `--bg-4` | `#2A2D33` | Pressed / selected fill, tooltips |

### Lines
| Token | Hex | Use |
|---|---|---|
| `--line-1` | `#22252A` | Hairline dividers between rows / regions |
| `--line-2` | `#2F333A` | Control borders, panel outlines |
| `--line-3` | `#454A53` | Hovered control borders |

### Text (warm neutrals, never pure white)
| Token | Hex | Contrast on `--bg-1` | Use |
|---|---|---|---|
| `--fg-1` | `#ECE9E3` | 15.6 : 1 | Primary text, values |
| `--fg-2` | `#A8A49C` | 7.6 : 1 | Secondary text, labels |
| `--fg-3` | `#7A776F` | 4.5 : 1 | Tertiary, captions, placeholders |
| `--fg-4` | `#4D4B47` | — | Disabled (never conveys information alone) |

### Accent — amber (restrained)
| Token | Value | Use |
|---|---|---|
| `--amber` | `#E5A23C` | Primary button, progress fill, focus ring, active nav marker |
| `--amber-hi` | `#F0B65C` | Primary hover |
| `--amber-lo` | `#C4852A` | Primary pressed |
| `--amber-ink` | `#1B1206` | Text on amber |
| `--amber-tint` | `rgb(229 162 60 / 0.10)` | Selected row wash, detected-platform chip |
| `--amber-line` | `rgb(229 162 60 / 0.38)` | Selected outlines |

Amber appears in at most three places on screen at once. It is never used for warnings.

### Status
| Token | Hex | Use |
|---|---|---|
| `--ok` | `#7FB894` | Completed, verified |
| `--err` | `#E0695C` | Failed, destructive |
| `--warn` | `#D7B95B` | Recoverable issues (retrying, rate-limited) |
| `--info` | `#86A8C8` | Neutral notices, "Other (via yt-dlp)" |

Status color is always paired with an icon or a word — never color alone.

### Light theme
Planned as a second token set (`[data-theme="light"]`); dark is the default and the design reference. The
"System" option follows `prefers-color-scheme`.

## 2. Typography

Faces (bundled, OFL-1.1, latin + latin-ext subsets): **IBM Plex Sans** 400/500/600 and **IBM Plex Mono** 400/500.
`font-variant-numeric: tabular-nums` is on for all mono text. No other faces.

| Token | Size / line | Weight | Face | Use |
|---|---|---|---|---|
| `t-micro` | 11 / 14, +0.06 em, uppercase | 500 | Sans | Column headers, section eyebrows |
| `t-meta` | 12 / 16 | 400 | Sans | Secondary metadata |
| `t-body` | 13 / 20 | 400 | Sans | Default UI text |
| `t-strong` | 13 / 20 | 500 | Sans | Row titles, button labels |
| `t-data` | 12 / 16 | 400 | Mono | Sizes, speeds, ETA, codecs |
| `t-data-lg` | 15 / 20 | 500 | Mono | Primary readouts (aggregate speed) |
| `t-input-hero` | 15 / 24 | 400 | Mono | URL field on Home |
| `t-title` | 18 / 24, −0.01 em | 600 | Sans | View titles |
| `t-display` | 24 / 30, −0.015 em | 600 | Sans | Media title in preview (2-line clamp) |

## 3. Space, size, radius

* **Grid:** 4 px. Scale: 2, 4, 8, 12, 16, 20, 24, 32, 40, 48.
* **Control heights:** 24 (inline/compact), 32 (default), 40 (prominent), 48 (hero URL field).
* **Window:** min 960 × 600, default 1180 × 760. Title bar 36. Sidebar 196. Content padding 24.
* **Radius:** 2 (tags, chips), 4 (buttons, inputs, rows), 6 (panels, menus), 8 (dialogs). Nothing larger; no pills
  except the 2 px tag radius.

## 4. Elevation

Flat by default — separation comes from surface steps and hairlines.

| Level | Treatment | Use |
|---|---|---|
| 0 | `--bg-1` | Canvas |
| 1 | `--bg-2` + 1 px `--line-1` | Rows, panels |
| 2 | `--bg-2` + 1 px `--line-2` + `0 8px 24px rgb(0 0 0 / .45)` | Menus, popovers, toasts |
| 3 | `--bg-2` + 1 px `--line-2` + `0 24px 64px rgb(0 0 0 / .6)` + scrim `rgb(0 0 0 / .5)` | Dialogs |

No glass, no blur, no glows.

## 5. Dividers

1 px `--line-1` between rows and regions; full-bleed inside panels, inset 24 px in content lists. Vertical
dividers in the title bar and stats strip use `--line-1` at 16 px height, centred.

## 6. Motion

| Token | Duration | Easing | Use |
|---|---|---|---|
| `m-quick` | 90 ms | `cubic-bezier(.2,.8,.2,1)` | Hover, press, color changes |
| `m-base` | 160 ms | same | Menus, tooltips, row insert |
| `m-slow` | 240 ms | same | View change (opacity 0→1 + 4 px rise), preview reveal |

Progress fills animate width with `m-base` linear to smooth 10 Hz updates. `prefers-reduced-motion: reduce`
disables translate/shimmer, keeps opacity changes ≤ 90 ms. No looping animation except the active-transfer
pulse on the status dot (1.6 s, opacity only, disabled under reduced motion).

## 7. Icons

Custom set in `src/icons/`, 16 × 16 grid, 1.5 px stroke, **square caps, miter joins**, `currentColor`, optical
padding 1.5 px. 20 px variant only for the title-bar controls. Set: `link, paste, clear, search, filter, sort,
download, pause, play, stop, retry, folder, file, external, copy, trash, check, alert, info, chevron-down,
chevron-right, home, queue, library, settings, video, audio, image, gallery, subtitle, minimize, maximize,
restore, close`. Platform marks are drawn as **monogram tags** (e.g. `YT`, `TT`, `BI`) in mono on a 2 px-radius
chip — Rabbit never ships third-party logos.

## 8. Components

### Buttons
| Variant | Rest | Hover | Pressed | Disabled |
|---|---|---|---|---|
| Primary | `--amber` fill, `--amber-ink` text | `--amber-hi` | `--amber-lo`, translateY(0.5px) | `--bg-3` fill, `--fg-4` text |
| Secondary | `--bg-3` fill, 1 px `--line-2`, `--fg-1` | border `--line-3` | `--bg-4` | `--fg-4` |
| Ghost | transparent, `--fg-2` | `--bg-3`, `--fg-1` | `--bg-4` | `--fg-4` |
| Danger | transparent, `--err` text | `rgb(224 105 92 / .1)` fill | darker | — |
| Icon | 28 × 28 ghost, icon 16 | as Ghost | as Ghost | — |

Labels are sentence case, verbs first ("Download 1080p", "Reveal in Explorer"). Keyboard shortcut hints render
as mono `kbd` chips (`--bg-4`, 2 px radius, `t-data`).

### Inputs
32 px, `--bg-3` fill, 1 px `--line-2`, 4 px radius, 0 10 px padding. Hover → `--line-3`. Focus → border `--amber`
+ 1 px inner `--amber-line` (no outer glow). Invalid → border `--err` + message row below in `t-meta` `--err`
with the alert icon. Placeholder `--fg-3`.

**Hero URL field (Home):** 48 px, mono 15 px, left slot holds the detected-platform monogram chip (appears with a
`m-base` fade when detection succeeds; `--info` "Other" chip for unknown hosts), right slot holds `Paste` (ghost,
`Ctrl+V` hint) and `Analyze` (primary). Multi-URL input expands to a 3-line textarea and shows "3 links found".

### Menus & selects
Level-2 surface, 4 px padding, items 28 px tall, `t-body`, hover `--bg-3`, checked item shows a 16 px check in
`--amber`. Keyboard: arrows, Home/End, type-ahead, Esc closes and restores focus.

### Format picker (preview)
A dense table, not cards: columns `Quality · Container · Codec · FPS · Size · Notes`. One selectable row
(radio semantics), selected row gets `--amber-tint` wash + 2 px `--amber` left rule. "Notes" shows real facts
only: "video only — merges with best audio", "estimated size", "conversion (FFmpeg)". Audio-only and
image/gallery options are separate tabs above the table: `Video · Audio · Images · Subtitles` — tabs that have no
real options are hidden, not disabled.

### Progress — the ruler
Track 4 px tall, `--bg-3`, ticks every 10 % as 1 × 4 px `--line-2` notches; fill `--amber` (active), `--fg-3`
(paused), `--ok` (verifying/completed), `--err` (failed, frozen at failure point). Indeterminate (size unknown):
a 20 % amber segment travelling at 1.2 s linear — replaced by a static hatched track under reduced motion. Below
the track, live segments render as 2 px micro-bars (one per segment, width proportional to range).

### Queue row (56 px)
`[thumb 64×36] [title / platform tag · format · uploader] [ruler + readouts: 41.2 / 128.0 MB · 12.4 MB/s · 0:07]
[actions]`. Actions are icon buttons revealed on hover/focus (pause/resume, cancel, more), always visible for
the selected row and for keyboard users.

### Tooltips
`--bg-4`, `--fg-1`, `t-meta`, 6 × 8 padding, 4 px radius, 400 ms delay, 90 ms fade, shortcut in `kbd` chip.

### Dialogs
Level 3, max-width 440, title `t-title`, body `t-body --fg-2`, actions right-aligned (secondary then primary or
danger). Destructive confirm requires the danger button to be focused second, never by default.

### Notifications
In-app toasts bottom-right, 320 wide, level 2, 3 px left status rule, auto-dismiss 5 s (errors persist until
dismissed), max 3 visible. Windows toast notifications only for completion/failure while the window is unfocused
(setting).

### Loading
* Analysis: hero field shows a 1 px amber line sweeping under the input + stage text and an elapsed timer in mono.
* Preview skeleton: blocks in `--bg-3` with a `--bg-4` shimmer (disabled under reduced motion).
* Never a full-screen spinner; app start renders the shell immediately.

### Empty states
One line of `--fg-2` text, one line of `--fg-3` guidance, one action. Example (Queue): "Nothing downloading." /
"Paste a link on Home or drop one anywhere in the window." / `Go to Home`. No illustrations.

### Error states
Inline, specific, actionable: "This Instagram story requires login. Rabbit doesn't sign in to platforms." /
"The download link expired. Rabbit re-checked the source and will retry." Raw extractor messages are available
behind "Details" (selectable mono text, copy button).

### Focus
`:focus-visible` → 2 px `--amber` outline, 2 px offset, 4 px radius. Mouse focus shows nothing extra. Focus order
follows reading order; the title bar is skipped via a "Skip to content" target.

### Scrollbars
10 px gutter (`scrollbar-gutter: stable`), transparent track, 6 px `--bg-4` thumb, `--line-3` on hover, fully
rounded (the one allowed full radius).

## 9. Layout & navigation

```
┌ title bar 36 ─────────────────────────────────────────────── ─  □  ✕ ┐
│ ◆ Rabbit                                   drag region                │
├ sidebar 196 ─┬ content ──────────────────────────────────────────────┤
│ Home     ⌃1  │  View title (t-title)               view actions      │
│ Queue  3 ⌃2  │  ─────────────────────────────────────────────────── │
│ Library  ⌃3  │                                                       │
│              │                                                       │
│              │                                                       │
│ ──────────── │                                                       │
│ ↓ 24.6 MB/s  │                                                       │
│ 3 active     │                                                       │
│ Settings ⌃,  │                                                       │
└──────────────┴───────────────────────────────────────────────────────┘
```
Active nav item: `--fg-1` text, `--bg-2` fill, 2 px `--amber` left marker. The sidebar footer shows the aggregate
transfer readout (mono, live) — the app's "heartbeat".

### Views
* **Home** — hero URL field centred in the upper third; below it the analysis result (Preview) slides in; beneath
  that "Recent" (last 6 analysed URLs, one line each, mono host + title). Drop target: whole window, outlined
  with a 1 px dashed `--amber-line` inset while dragging.
* **Preview** (inside Home) — left 320 px thumbnail (16:9 letterboxed on `--bg-0`; galleries show a 3 × 2 grid
  with "+N"), right column: platform tag, title (`t-display`), uploader · duration · upload date (`t-data`), tabs
  + format table, footer bar with destination folder (click to change), filename preview, primary
  `Download <label>` button.
* **Queue** — filter segmented control `All · Active · Queued · Done · Failed`, list of queue rows, bulk actions
  (Pause all / Resume all / Clear finished).
* **Library** — search field (FTS), filters (platform, kind, date), sort, virtualised table: thumb, title,
  platform, format, size, date; row actions open / reveal / copy URL / re-download / remove.
* **Settings** — single scrolling page with sticky section index on the left: Downloads, Network, Formats,
  Files, Behaviour, Appearance, Tools (yt-dlp/FFmpeg versions, update), Data, About.

## 10. Keyboard

| Shortcut | Action |
|---|---|
| `Ctrl+V` (anywhere, not in a field) | Paste into URL field and analyze |
| `Ctrl+L` | Focus URL field |
| `Enter` (URL field) | Analyze; in preview: Download selected |
| `Ctrl+1/2/3` | Home / Queue / Library |
| `Ctrl+,` | Settings |
| `Space` (queue row) | Pause / resume |
| `Delete` (queue row) | Cancel (with confirm if > 0 bytes) |
| `Ctrl+O` / `Ctrl+Shift+O` | Open file / Reveal in Explorer (selected row) |
| `Ctrl+F` | Search Library |
| `Esc` | Close menu/dialog, clear field |

## 11. Copy voice

Plain, specific, no exclamation marks, no emoji. Numbers with units in mono ("128.0 MB", "1920×1080",
"0:07 left"). Never "Oops". Never claim "no watermark" or "highest quality" unless the extracted metadata says so.
