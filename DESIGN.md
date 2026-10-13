---
name: rpodder
description: A self-hosted gpodder sync server whose web UI reads as a library card catalog you own.
colors:
  ground: "#e8ebe5"
  panel: "#fcfcf9"
  panel-2: "#f2f4ef"
  sunk: "#f6f7f3"
  ink: "#1a201d"
  ink-2: "#47524c"
  ink-3: "#5f6b64"
  rule: "#d3d9d1"
  rule-strong: "#b3bdb3"
  card-rule: "#c6d6e8"
  shell: "#1c4436"
  shell-2: "#24533f"
  shell-ink: "#f0f5f1"
  shell-ink-2: "#b5cbbf"
  primary: "#1d6a50"
  primary-hover: "#175a43"
  primary-ink: "#ffffff"
  primary-soft: "#d9eae1"
  red: "#bf3a2c"
  red-soft: "#f7e1dc"
  brass: "#7f611d"
  brass-soft: "#f0e6cc"
  danger: "#b0261d"
  danger-soft: "#f8dfdc"
  success: "#24724a"
  warning: "#8a5a00"
  warning-soft: "#f6e8c8"
  group-1: "#2c7a58"
  group-2: "#2d5c9a"
  group-3: "#7c3a6c"
  group-4: "#9a6210"
typography:
  display:
    fontFamily: "'Atkinson Hyperlegible Next Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "2.25rem"
    fontWeight: 700
    lineHeight: 1.08
    letterSpacing: "-0.03em"
  headline:
    fontFamily: "'Atkinson Hyperlegible Next Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.75rem"
    fontWeight: 700
    lineHeight: 1.25
    letterSpacing: "-0.02em"
  title:
    fontFamily: "'Atkinson Hyperlegible Next Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 700
    lineHeight: 1.5
  body:
    fontFamily: "'Atkinson Hyperlegible Next Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.5
  body-sm:
    fontFamily: "'Atkinson Hyperlegible Next Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.43
  label:
    fontFamily: "'Atkinson Hyperlegible Next Variable', ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 600
    lineHeight: 1.4
  data:
    fontFamily: "'Atkinson Hyperlegible Mono Variable', ui-monospace, monospace"
    fontSize: "0.75rem"
    fontWeight: 400
    letterSpacing: "-0.01em"
    fontFeature: "'tnum' 1"
  stamp:
    fontFamily: "'Atkinson Hyperlegible Mono Variable', ui-monospace, monospace"
    fontSize: "0.6875rem"
    fontWeight: 600
    letterSpacing: "0.04em"
rounded:
  hairline: "2px"
  stamp: "3px"
  sm: "4px"
  card: "6px"
  full: "9999px"
spacing:
  xs: "6px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
  2xl: "32px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.primary-ink}"
    typography: "{typography.label}"
    rounded: "{rounded.card}"
    padding: "0 14.4px"
    height: "36px"
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
  button-secondary:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "0 14.4px"
    height: "36px"
  button-secondary-hover:
    backgroundColor: "{colors.panel-2}"
  button-quiet:
    textColor: "{colors.ink-2}"
    rounded: "{rounded.card}"
    padding: "0 14.4px"
    height: "36px"
  button-quiet-hover:
    backgroundColor: "{colors.panel-2}"
    textColor: "{colors.ink}"
  button-danger:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.danger}"
    rounded: "{rounded.card}"
    height: "36px"
  button-danger-hover:
    backgroundColor: "{colors.danger-soft}"
  button-sm:
    padding: "0 10.4px"
    height: "30px"
  play-button:
    textColor: "{colors.primary}"
    rounded: "{rounded.full}"
    size: "36px"
  play-button-loaded:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.primary-ink}"
    rounded: "{rounded.full}"
    size: "36px"
  field:
    backgroundColor: "{colors.sunk}"
    textColor: "{colors.ink}"
    rounded: "{rounded.card}"
    padding: "0 12px"
    height: "38px"
  panel:
    backgroundColor: "{colors.panel}"
    rounded: "{rounded.card}"
  catalog-card:
    backgroundColor: "{colors.panel}"
    rounded: "{rounded.card}"
    padding: "24px"
  tag:
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "0 8px"
    height: "22px"
  tag-brass:
    backgroundColor: "{colors.brass-soft}"
    textColor: "{colors.brass}"
    rounded: "{rounded.sm}"
  stamp:
    typography: "{typography.stamp}"
    rounded: "{rounded.stamp}"
    padding: "0 6.4px"
    height: "20px"
  shell-bar:
    backgroundColor: "{colors.shell}"
    textColor: "{colors.shell-ink}"
    height: "56px"
  cover:
    backgroundColor: "{colors.panel-2}"
    rounded: "{rounded.sm}"
    size: "56px"
---

# Design System: rpodder

## Overview

**Creative North Star: "The Reading Room Catalog"**

The web UI is a library card catalog for the podcasts you own. Every feed is a catalog card: its feed address is the call number, the devices that carry it are its holdings, and plays are stamped onto it like a circulation slip. The room has two lightings. Day is pale stock-card ground, index-card white panels, typewriter-black ink, faint blue card rules and a library-green shell bar. Night is the same room: green-black ground, panels one step lighter, low-contrast green-grey rules, red and brass brightened. Light and dark are full peers. The page follows the system preference unless `data-theme` pins one.

Density is that of a working catalog. Rows are compact, covers sit at fixed sizes on a shared baseline, durations are drawn to a common scale, and nothing is labelled twice. Hierarchy comes from scale and weight in one workhorse sans. Colour mostly sits in a few small places: green for action, a red index rule for "this card" and "now playing", brass for counts and filing letters. The build has no glass, no gradients and no album-art mosaics as decoration.

**Key Characteristics:**
- One sans (Atkinson Hyperlegible Next) for all UI; its mono sibling only for data.
- Hairline borders and 6px square-ish corners; depth from tonal steps, with shadow reserved for lifted objects.
- A red 3px rule marks the card head, the active tab and playback progress.
- Holdings marks: one small square per device, filled when that device holds the feed, coloured by sync group.
- State is stamped and stays visible (Played, 12 min left, Downloaded); it never just disappears.

## Colors

A restrained reading-room palette: green-tinted neutrals carry almost every surface, with three small, named accents.

### Primary
- **Library Green** (primary): every primary action (Add feed, Subscribe, Sign in), the filled play button, focus rings, the caret and checkbox/radio accents. Hover deepens to **Stack Green** (primary-hover). **Pale Green Wash** (primary-soft) is the tinted fill for green-toned surfaces.
- **Shell Green** (shell, shell-2): the sticky top bar only. Its ink pair (shell-ink, shell-ink-2) is used for active and resting nav labels. Shell green never fills content surfaces.

### Secondary
- **Catalog Red** (red): the index rule. It is the 3px top border of every catalog card, the active-tab underline in the shell bar, Discover tabs and phone tab bar, the mini-player progress rule, Continue-listening progress fills, range-input accents and the "min left" / "Marked new" stamps. **Red Wash** (red-soft) is its tint.

### Tertiary
- **Ledger Brass** (brass) on **Brass Card** (brass-soft): counts beside headings (Users 12, Your devices 4, episode totals, tag usage), the A–Z drawer letters in the Library, the Admin tag, the "Downloaded" stamp, the letter-tile fallback for missing covers, and text selection.

### Neutral
- **Stock Card Ground** (ground): page background behind everything.
- **Index Card** (panel): every panel, card, row list, menu, toast and the docked player.
- **Card Shade** (panel-2): hover fill for rows and quiet buttons, skeletons, cover placeholder, segmented-control track.
- **Inkwell Sunk** (sunk): input fields, set slightly below the panel.
- **Typewriter Ink** (ink), **Faded Ink** (ink-2), **Pencil** (ink-3): primary text, secondary text (authors, descriptions), tertiary text (field names, timestamps, placeholders).
- **Hairline** (rule), **Drawer Edge** (rule-strong): panel borders and dividers; rule-strong also borders fields and secondary buttons and separates Library drawers.
- **Card Blue Rule** (card-rule): the faint blue hairline inside a card, between rows within a drawer, between episode and history rows, and above a card's field list.

### Status and groups
- **danger / danger-soft**, **success**, **warning / warning-soft**: destructive actions, confirmations (Played, copied), and the HTTPS-upgrade notice icon.
- **group-1 to group-4** (green, blue, plum, ochre): sync-group colours for holdings marks and device legends, assigned by group index modulo four. Ungrouped devices use ink-2 / ink-3.

### Named Rules
**The Red Line Rule.** Catalog Red is a rule, not a fill. It appears as a 3px line (card head, active tab, progress) or as stamp ink. It never colours a button or a large area.

**The Brass Counts Rule.** Numbers that count a collection and the letters that file it are brass mono. Primary actions are never brass.

**The Same Room Rule.** Every token has a night value with the same role. Dark mode is not inverted. Ground goes darkest, panels go one step lighter, and accents get brighter.

## Typography

**Display Font:** Atkinson Hyperlegible Next Variable (with ui-sans-serif, system-ui)
**Body Font:** Atkinson Hyperlegible Next Variable
**Label/Mono Font:** Atkinson Hyperlegible Mono Variable (with ui-monospace), for data only

**Character:** Built for legibility, the sans is plain and warm without being generic. Its mono sibling reads like a typewritten catalog card wherever the content is a machine value. Both are self-hosted via @fontsource-variable.

### Hierarchy
- **Display** (700, 2.25rem rising to 2.75rem at sm, 1.08, -0.03em): the signed-out landing headline only.
- **Headline** (700, 1.75rem, 1.25, -0.02em): one page title per route (Library, Discover, History, Devices, Admin, Settings). The podcast catalog card title runs 1.625rem to 1.875rem at the same tracking; auth cards use 1.5rem.
- **Title** (700, 1rem): section headings inside a page (Continue listening, Subscriptions, Your devices). Empty-state and landing section titles step up to 1.125rem.
- **Body** (400, 15px, 1.5): base reading size. Long descriptions cap at 70–75ch with relaxed leading.
- **Body small** (400, 0.875rem): row metadata, buttons, notices, menus; the most-used size in the build. Row titles are this or body at 600.
- **Label** (600, 0.8125rem, ink-2): form labels, and the field names in a card's definition list (in ink-3).
- **Data** (mono, tabular numerals, -0.01em, usually 0.75–0.8125rem): feed URLs, durations, clock times, dates, counts, device ids, build info, playback rate.
- **Stamp** (mono 600, 0.6875rem, 0.04em, uppercase): state stamps only.

### Named Rules
**The Mono Is Data Rule.** Mono is used only for values a machine produced or a person would copy: URLs, times, counts, ids. Headings, labels and prose are never mono.

**The Scale and Weight Rule.** Hierarchy comes from size and weight (400 / 600 / 700) in one family, and from ink-2 / ink-3 for recession. Don't add a second display face or colour headings.

## Layout

One centred column, max 72rem (`max-w-6xl`) with 16px side padding, under a sticky 56px shell bar. Content starts 24px below the bar (32px from sm). Major page sections stack 32px apart. A section heading sits 12px above its content. Card padding is 16–20px for forms and add-feed panels, 20–24px for the podcast card, and 24–32px for empty states and auth.

Lists are dense rows inside a single panel, separated by hairlines rather than gaps. A Library row is 10px vertical padding, a guide-letter gutter (36–44px), a 56px cover, title and author, holdings marks and the last-played date, right-aligned in a 96px mono column. Covers are fixed sizes, never fluid inside rows: 36px (phone player), 48px (slips, desktop player), 56px (rows), 144–160px (podcast card), and full-width square only in grids and the phone now-playing sheet.

Responsive behaviour: at md (768px) and up, navigation lives in the shell bar. Below md, a fixed 56px bottom tab bar takes over (icon over label), and the mini player docks above it. Main content reserves bottom padding for both. On phones, tapping the mini player opens a full-screen now-playing sheet. Continue-listening slips grid 1 / 2 / 3 columns at base / sm / lg. Cover grids run 3 / 4 / 6 columns.

### Named Rules
**The Shared Baseline Rule.** Covers in a list are one fixed size, so the titles line up. Never size a cover by its content.

**The True Scale Rule.** When durations are drawn, they share one scale per view (the longest item sets 100%), so a short episode looks short.

## Elevation & Depth

The build is mostly flat, with tonal layering. Depth comes from ground → panel → panel-2 steps and hairline borders. Two shadow tokens exist and are used sparingly, always soft and ambient:

### Shadow Vocabulary
- **Rest lift** (`--shadow`, light: `0 1px 2px rgb(20 30 25 / 0.06), 0 4px 16px rgb(20 30 25 / 0.06)`): the podcast catalog card, the landing example card, the auth card, the docked mini player, the selected segment of a segmented control, and the large play button in the phone sheet.
- **Floating** (`--shadow-lift`, light: `0 2px 6px rgb(20 30 25 / 0.1), 0 12px 32px rgb(20 30 25 / 0.12)`): things that float over content, such as the account menu popover, toasts and the cover in the phone now-playing sheet.

Dark mode swaps both for black-based values with higher alpha.

### Named Rules
**The Flat Rows Rule.** Rows, slips, list panels and form cards carry no shadow. A shadow means "this is the record you came to see" or "this floats above the page".

## Shapes

The corners are square-ish and the edges hairline. The card radius is 6px and is shared by panels, buttons, fields, notices, menus and toasts. Smaller elements step down: 4px for tags, covers, menu items and skeletons; 3px for stamps; 2px for holdings marks and the caps of the 3px indicator rules. Circles appear only on play buttons and the phone-sheet skip buttons. Every border is 1px except the 3px red index rule. The catalog card squares off its top corners (radius `0 0 6px 6px`) so the red rule runs flat across its head, like the coloured edge of an index card.

Holdings marks are 8–10px squares. A filled mark means the device holds the feed. An outlined mark means it doesn't. A dashed outline on a device tag means "not held".

## Components

### Buttons
Restrained, tactile, all one height.
- **Shape:** card radius (6px), 36px tall, 14.4px horizontal padding, 600 weight at 0.875rem, 6.4px icon gap. Small: 30px tall at 0.8125rem. Icon-only buttons are square.
- **Primary:** Library Green fill with white ink. At most one per action group.
- **Hover / Focus:** a 150ms ease-out colour shift to primary-hover. Focus-visible draws a 2px primary outline at 2px offset. Disabled drops to 55% opacity with a not-allowed cursor.
- **Secondary:** panel fill, rule-strong border, ink text; hover fills panel-2. Used for Export OPML, Subscribed (with a green bookmark icon), "Browse the directory".
- **Quiet:** transparent with ink-2 text; hover fills panel-2 with ink text. Used for icon actions, Refresh feed, Website, dismiss.
- **Danger:** panel fill, a danger border mixed 45% into the rule, danger text; hover fills danger-soft.
- **Play button:** a circle of 36px (44px large). An idle episode gets a green outline with green glyph. Loaded or playing fills it green. An item with no audio gets a rule outline with faded glyph and a not-allowed cursor.

### Chips (tags and stamps)
- **Tag:** 22px tall, 4px radius, 0.75rem 600. Variants: brass (brass-soft fill, brass ink) for Admin and drawer letters. Device tags in a card get a rule-strong border on panel when held, or a dashed rule border with ink-3 when not, led by a holdings mark.
- **Stamp:** 20px tall, 1px border in currentColor, 3px radius, mono 600 uppercase at 0.6875rem. The tone is the text colour: success for Played, red for "N min left" and Marked new, primary for Played in history, brass for Downloaded, ink-3 for Deleted, danger for Inactive.

### Cards / Containers
- **Catalog card:** panel fill, 1px rule border, a 3px Catalog Red top border, radius `0 0 6px 6px`. It is the head of the podcast page and the frame for auth, add-feed, admin forms and empty states. Card fields are a two-column definition list (field name in ink-3 600, value right), set under a card-rule hairline.
- **Panel:** panel fill, 1px rule border, 6px radius. It holds row lists (divided by rule or card-rule hairlines), Continue-listening slips and the device lists.
- **Notice:** a panel-styled flex row, 12px by 16px padding, 0.875rem, led by a tone-coloured icon.
- **Skeleton:** panel-2 blocks at 4px radius, pulsing opacity over 1.4s. They mirror the real layout (cover square, two text bars).
- **Shadow Strategy:** see Elevation & Depth. Only the head catalog card on a page carries rest lift.

### Inputs / Fields
- **Style:** 38px tall, sunk fill, 1px rule-strong border, 6px radius, 12px horizontal padding, 0.9375rem; placeholder in ink-3. Selects use a custom chevron and 32px right padding. A search filter sits at 36px with a leading 16px icon.
- **Focus:** the border turns primary and gets a 3px primary halo at 22% mix. There is no outline.
- **Labels and hints:** labels are 0.8125rem 600 ink-2, 5.6px above the field. Hints are 0.8125rem ink-3.
- **Disabled:** 60% opacity.

### Navigation
- **Shell bar:** sticky, 56px, shell green. Logo and the "rpodder" wordmark (1.0625rem 700) sit left, account menu right. Nav links are 0.875rem 600 in shell-ink-2, going to shell-ink on hover and when active. The active link has a 3px red rule on the bar's bottom edge with 2px top caps. The theme switch lives in the account menu (a popover panel with floating shadow), not in the bar.
- **Sub-tabs (Discover):** same pattern on panel ground, ink-2 to ink, with the red rule sitting on the tab strip's hairline.
- **Phone tab bar:** fixed bottom, panel fill, top hairline, four equal cells with a 20px Lucide icon over a 0.6875rem 600 label. Active is ink, with a 3px red rule on the top edge. Resting is ink-3.
- **Theme switch:** a segmented control (panel-2 track, rule border, 6px). The selected segment is panel with rest lift.

### Holdings Marks (signature)
A row of 8–10px squares with 2px corners and 4px gap, one per device. The border is in the device's sync-group colour, with a solid fill when that device holds the feed and empty when it doesn't. A screen-reader summary sits alongside, and the Library shows a legend above the list. The same marks appear at larger scale inside device tags on the catalog card.

### Mini Player (signature)
Docked to the bottom (above the phone tab bar), panel fill, top hairline, rest lift. A 3px red progress rule runs across its top edge over a rule-coloured track. The bar is 64px on phones and 72px from md. The 40px round play button is green; skip ±15/30 are quiet icon buttons with the seconds in tiny mono. Times and rate are mono. On phones it expands into a full-screen now-playing sheet that slides up over 220ms with `cubic-bezier(0.16, 1, 0.3, 1)`.

### Continue-listening Slip
A panel with a 48px cover, title, podcast link and a 4px rounded track. The track width is drawn to true duration scale against the longest slip, with a red fill for position, followed by mono "N min left". A play button sits on the right.

### Motion
Colour transitions run 150ms ease-out. Progress width runs 200ms ease-out. The phone sheet entrance is 220ms. Skeleton pulse is 1.4s. Under `prefers-reduced-motion`, all animation and transition durations collapse to near zero.

## Do's and Don'ts

### Do:
- **Do** take every colour from the CSS variables in `web/src/app.css` so both lightings follow. Never hard-code a hex in markup.
- **Do** set feed URLs, durations, dates, counts and ids in the mono `data` style with tabular numerals.
- **Do** head the primary record of a page with the catalog card (3px red top rule, `0 0 6px 6px` radius) and give it rest lift.
- **Do** show which devices hold a feed with holdings marks in sync-group colours wherever a feed is listed.
- **Do** stamp state (Played, N min left, Downloaded, Inactive) with the bordered mono stamp, and keep stamped items visible.
- **Do** show an explicit "—" when a value is unknown (for example, partial history), and only state a negative ("Not played") when the data is complete.
- **Do** keep covers at fixed sizes (36 / 48 / 56 / 144–160px) within a list.
- **Do** use Lucide line icons at 16px in buttons and 20px in nav and player.

### Don't:
- **Don't** fill buttons or large areas with Catalog Red. It is a 3px rule and stamp ink only.
- **Don't** use glass, backdrop blur or gradients. Surfaces are flat panels with hairlines.
- **Don't** use shadows on rows, slips or list panels, and don't use hard offset shadows anywhere.
- **Don't** introduce a second display typeface or set headings, labels or prose in mono.
- **Don't** use uppercase letter-spaced labels above headings. Uppercase belongs only to state stamps.
- **Don't** use a stamp as a decorative or section label. Stamps record state.
- **Don't** put the theme switch or other settings in the shell bar. The bar carries navigation only.
- **Don't** show a dark album-art wall with a neon accent, or a grey admin-dashboard look. Covers are fixed-scale catalog thumbnails, not the layout.
