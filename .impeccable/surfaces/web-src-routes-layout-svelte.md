---
version: 1
slug: "web-src-routes-layout-svelte"
primary_target: "web/src/routes/+layout.svelte"
related_targets: ["web/src/routes"]
---

# rpodder web UI (all authenticated and public routes)

Scope: the whole embedded SPA, shell plus every route (library, discover, podcast, history, devices, settings, admin, auth pages). Visitor mode: **Operate**. Built unattended on 2026-10-12; the owner reviews in the morning.

Audience and job: self-hosters who sync AntennaPod / gPodder / Kasts through their own rpodder; they visit to check and fix sync state (subscriptions per device, sync groups), add feeds (search or raw URL), export OPML, read history, and listen in the browser with resume.

Constraints: data from the gpodder API only; no runtime CDN (fonts bundled); Pi-sized footprint; phone widths; light and dark themes (system default plus manual switch); keep every existing behaviour of the web player and sync logic.

Requested features: per-episode download link, OPML export (all and per device), add podcast by URL, device and sync-group management, light theme, player polish, Refresh feed only for admins.

## Direction contract

THESIS: Your podcasts as a library catalog you own. Every feed is a catalog card: its call number is the feed URL, its holdings are the devices that carry it, its circulation slip is the stamped record of plays. Refuses the dark album-art grid with a neon accent and the grey admin dashboard.

OWN-WORLD: Reading-room palette. Light: pale stock card ground, index-card white panels, typewriter-black ink, faint blue card rules as hairlines, library-green shell bar and primary actions, catalog-red top rule marking the card and "now playing", brass for tabs and counts. Dark: the same room at night, green-black ground, card panels a step lighter, rules low-contrast green-grey, red and brass brightened. One workhorse sans (Atkinson Hyperlegible Next) for all UI; its mono sibling only for data (call numbers, durations, timestamps, device ids). Lucide line icons, one stroke. Square-ish 6px corners, hairline borders, no glass, no gradients.

STORY: The visitor sees at once what they subscribe to and which devices hold each feed, resumes an episode, adds a feed by search or URL, exports OPML, and fixes devices and sync groups knowing exactly which devices an action touches.

FIRST VIEWPORT: Library (logged-in home). Green top bar: logo + rpodder, nav (Library, Discover, History, Devices, Admin), account menu holding the theme switch (kept out of the bar so the shell carries navigation only; also in Settings). Below: page title "Library" with counts, actions row (Export OPML, Add feed button that opens the feed-address field in place, so the header stays one line; Discover's search also accepts a pasted feed address). Continue listening strip of up to 6 slips with progress at true duration scale. Then the catalog: dense rows, 56px cover at fixed scale, title, author, holdings marks (one mark per device, filled when that device holds the feed, sync-group colour), last played. Mini player docked at the bottom with a red progress rule.

FORM: Library card catalog, candidate 6 of 7 (seed c7d8a226). Signature move: the catalog card header on the podcast page (red top rule, call number with copy, holdings by device, circulation stamps) and its compact holdings marks in every list. Raises: Labanotation: durations drawn to scale. Botanical folio: one fixed cover scale on a shared baseline. Saville catalog: nothing labelled twice. Vertical feed: on phones the player expands to a full now-playing sheet. Ticket wallet: state stamps itself (played, new) instead of disappearing. Type specimen: hierarchy by scale and weight only, one accent.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
