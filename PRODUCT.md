# Product

<!-- impeccable:product-schema 1 -->

> Written unattended on 2026-10-12 while the owner was away ("procedi con impeccable in autonomia"). Facts come from the repository (README, CLAUDE.md, docs/, the code) and the owner's own requests in that session. Lines marked *(inferred)* were not confirmed and should be checked.

## Platform

web

## Users

- **Self-hosters who listen to podcasts on several devices.** One person or a household runs rpodder on a home server or a Raspberry Pi, points AntennaPod, gPodder or Kasts at it, and opens the web UI to see and manage what those apps synchronize. They are technical enough to run Docker, and they treat the web UI as the control room for their own sync server rather than a destination app.
- **The admin of the instance** (often the same person): creates users, checks the server, forces feed refreshes.
- **Household members or friends** with their own account on someone else's instance *(inferred from multi-user support, open/invite registration)*.

## Product Purpose

rpodder replaces gpodder.net (mygpo): it stores podcast subscriptions and episode actions (play position, download, delete) so every client of the gpodder API stays in sync. The web UI lets a user see their subscriptions, discover and add podcasts, manage devices and sync groups, read listening history, and since the web player shipped, listen in the browser and resume where the phone left off. Success: the user trusts that what they see in the browser is exactly what their apps will sync, and can fix or extend it (add a feed, move a device into a group, export their OPML) without touching the apps.

## Positioning

Self-hosted, open and gpodder-API compatible: the user's subscriptions and listening history live on their own server, readable and exportable, and every existing gpodder client keeps working. The web player can itself be a sync device, so the browser is one more place to listen, not a separate silo.

## Operating Context

- Clients: AntennaPod, gPodder, Kasts (tested), any gpodder-API app. They sync in the background; the web UI is visited occasionally to check, manage or listen.
- Deployment: single Rust binary or Docker image with the Svelte UI embedded; SQLite on a Raspberry Pi or PostgreSQL for bigger instances; optional SSO (Authentik / OIDC), optional SMTP, optional Podcast Index for external search and trending.
- Feed URLs can carry private tokens (Patreon, premium feeds); those are hidden from the public directory and only shown to subscribers.
- Episode media are played straight from the publisher (no proxy); plain-HTTP media on an HTTPS instance may fail to play, in which case the UI offers the file directly.

## Capabilities and Constraints

- Subscriptions per device, sync groups that share subscriptions, episode actions with positions, listening history, favorites, chapters, podcast lists, settings, tags, directory/toplist/search, Podcast Index search and trending, HTTPS upgrade suggestions, admin user management, OPML/TXT/JSON export of subscriptions.
- Web player: one `rpodder-web` device per user; optional position sync (account setting); optional membership in a sync group.
- Requested by the owner for the UI overhaul: per-episode download link, OPML export link, adding a podcast by feed URL, device and sync-group management, a light theme next to the dark one, a reorganized and more polished interface.
- Stack: Svelte 5 + SvelteKit (adapter-static, SPA, `ssr=false`) + Tailwind v4, embedded in the Rust binary. Data loading in `$effect` with a `browser` guard. No external CDN at runtime *(inferred: self-hosted, offline-friendly)*.
- Must run well on a Raspberry Pi-hosted instance and on phones: small bundle, no heavy runtime dependencies.

## Brand Commitments

- Name: **rpodder**, written lowercase.
- Logo: `web/static/logo.svg`, a gear-toothed headphone mark (black fill, inverted on dark).
- Documentation lives at https://thekoma.github.io/rpodder/.
- Interface language: English.

## Evidence on Hand

- Podcast artwork and metadata come from the feeds themselves (logo URLs, titles, authors, descriptions); nothing is bundled.
- No testimonials, user counts or benchmarks exist; none may be invented.

## Product Principles

1. **The browser shows exactly what the apps sync.** Device, group and subscription state is never approximated or hidden; destructive actions say which devices they touch.
2. **The user owns their data.** Export (OPML), feed URLs and media files are one click away.
3. **Quiet control room, then player.** Managing sync is occasional and must be fast and legible; listening is the one place the UI may linger.
4. **Works on the small server and the small screen.** Raspberry Pi footprint, phone-width layouts, no runtime CDNs.

## Accessibility & Inclusion

No product-specific requirement was established; WCAG AA contrast and full keyboard operation are the working floor *(inferred)*.
