#!/usr/bin/env python3
"""
Mock RSS server for rpodder integration tests.

Endpoints:
  /feed/static   — Always returns the same 2-episode RSS feed.
                   Supports ETag: returns 304 on second request.
  /feed/dynamic  — Returns a feed with N+1 episodes each request
                   (starts with 1, then 2, then 3...).
  /feed/player   — 3-episode feed for the web player: a playable WAV, a 404
                   enclosure, and an item with only a <link> page.
  /feed/repaired — item "rep-1" with only a <link> page until /feed/repaired/fix,
                   then with an <enclosure> too; item "rep-2" only ever has a
                   <link> (text post). Mimics what 2026.10.1-2026.10.2 stored.
  /media/tone.wav    — 180 s playable WAV (440 Hz tone), supports Range.
  /media/missing.mp3 — 404.
  /stats         — JSON counters: how many times each feed was fetched
                   and how many 304s were returned.
  /reset         — Reset counters and dynamic state.

Environment:
  MOCK_PUBLIC_URL — base URL written into /feed/player enclosures, i.e. the
                    address a browser uses to reach this server
                    (default: http://host.docker.internal:8888).

Usage:
  python3 mock-rss.py [port]     (default: 8888)
"""

import io
import json
import math
import os
import re
import sys
import threading
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# Global state
state = {
    "dynamic_episode_count": 1,
    "repaired_fixed": False,
    "stats": {
        "static_fetched": 0,
        "static_304": 0,
        "dynamic_fetched": 0,
    },
}

# The server is threaded (a browser streams audio while the backend fetches
# feeds), so every access to `state` holds this lock.
state_lock = threading.Lock()

STATIC_ETAG = '"static-feed-v1"'
PUBLIC_URL = os.environ.get("MOCK_PUBLIC_URL", "http://host.docker.internal:8888").rstrip("/")
TONE_SECONDS = 180


def make_tone_wav(seconds=TONE_SECONDS, rate=8000, freq=440):
    buf = io.BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(1)
        w.setframerate(rate)
        w.writeframes(
            bytes(
                128 + int(60 * math.sin(2 * math.pi * freq * i / rate))
                for i in range(seconds * rate)
            )
        )
    return buf.getvalue()


TONE_WAV = make_tone_wav()


def make_episode(n, prefix="static"):
    return f"""    <item>
      <title>Episode {n}</title>
      <description>Description for episode {n}</description>
      <guid>{prefix}-ep-{n}</guid>
      <pubDate>Mon, {n:02d} Mar 2026 10:00:00 +0000</pubDate>
      <enclosure url="http://host.docker.internal:8888/audio/{prefix}-ep{n}.mp3"
                 type="audio/mpeg" length="1234567"/>
    </item>"""


def make_feed(title, episodes_xml):
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
<channel>
  <title>{title}</title>
  <link>http://host.docker.internal:8888</link>
  <description>Mock feed for testing</description>
  <language>en</language>
{episodes_xml}
</channel>
</rss>"""


def make_player_feed():
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
<channel>
  <title>Player Feed</title>
  <link>{PUBLIC_URL}</link>
  <description>Mock feed for the web player</description>
  <language>en</language>
    <item>
      <title>Player ep1</title>
      <description>A real, playable tone</description>
      <guid>player-ep-1</guid>
      <pubDate>Wed, 03 Jun 2026 10:00:00 +0000</pubDate>
      <itunes:duration>{TONE_SECONDS}</itunes:duration>
      <enclosure url="{PUBLIC_URL}/media/tone.wav" type="audio/wav" length="{len(TONE_WAV)}"/>
    </item>
    <item>
      <title>Player ep2</title>
      <description>Enclosure that returns 404</description>
      <guid>player-ep-2</guid>
      <pubDate>Tue, 02 Jun 2026 10:00:00 +0000</pubDate>
      <enclosure url="{PUBLIC_URL}/media/missing.mp3" type="audio/mpeg" length="1234"/>
    </item>
    <item>
      <title>Player ep3</title>
      <description>No enclosure, only a web page</description>
      <guid>player-ep-3</guid>
      <pubDate>Mon, 01 Jun 2026 10:00:00 +0000</pubDate>
      <link>{PUBLIC_URL}/page/ep3</link>
    </item>
</channel>
</rss>"""


def make_repaired_feed(fixed):
    enclosure = (
        f'<enclosure url="{PUBLIC_URL}/audio/rep1.mp3" type="audio/mpeg" length="100"/>'
        if fixed
        else ""
    )
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
<channel>
  <title>Repaired Feed</title>
  <link>{PUBLIC_URL}</link>
  <description>Enclosure missed, then found</description>
    <item>
      <title>Rep ep1</title>
      <guid>rep-1</guid>
      <pubDate>Wed, 03 Jun 2026 10:00:00 +0000</pubDate>
      <link>{PUBLIC_URL}/page/rep1</link>
      {enclosure}
    </item>
    <item>
      <title>Rep ep2</title>
      <guid>rep-2</guid>
      <pubDate>Tue, 02 Jun 2026 10:00:00 +0000</pubDate>
      <link>{PUBLIC_URL}/page/rep2</link>
    </item>
</channel>
</rss>"""


class MockHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        # Suppress default logging
        pass

    def do_GET(self):
        if self.path == "/feed/static":
            self.handle_static()
        elif self.path == "/feed/dynamic":
            self.handle_dynamic()
        elif self.path == "/feed/player":
            body = make_player_feed().encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/rss+xml")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/feed/repaired":
            with state_lock:
                fixed = state["repaired_fixed"]
            body = make_repaired_feed(fixed).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/rss+xml")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/feed/repaired/fix":
            with state_lock:
                state["repaired_fixed"] = True
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"status":"fixed"}')
        elif self.path == "/media/tone.wav":
            self.handle_tone()
        elif self.path == "/stats":
            self.handle_stats()
        elif self.path == "/reset":
            self.handle_reset()
        elif self.path.startswith("/audio/"):
            # Fake audio file
            self.send_response(200)
            self.send_header("Content-Type", "audio/mpeg")
            self.end_headers()
            self.wfile.write(b"\x00" * 100)
        else:
            self.send_error(404)

    def handle_tone(self):
        size = len(TONE_WAV)
        start, end = 0, size - 1
        m = re.match(r"bytes=(\d*)-(\d*)$", self.headers.get("Range", ""))
        if m and (m.group(1) or m.group(2)):
            if m.group(1):
                start = int(m.group(1))
                end = min(int(m.group(2)), size - 1) if m.group(2) else size - 1
            else:
                start = max(size - int(m.group(2)), 0)
            if start > end or start >= size:
                self.send_response(416)
                self.send_header("Content-Range", f"bytes */{size}")
                self.end_headers()
                return
            self.send_response(206)
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        else:
            self.send_response(200)
        self.send_header("Content-Type", "audio/wav")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(end - start + 1))
        self.end_headers()
        try:
            self.wfile.write(TONE_WAV[start : end + 1])
        except (BrokenPipeError, ConnectionResetError):
            pass

    def handle_static(self):
        # Check If-None-Match for conditional GET
        if_none_match = self.headers.get("If-None-Match", "")
        if if_none_match == STATIC_ETAG:
            with state_lock:
                state["stats"]["static_304"] += 1
            self.send_response(304)
            self.send_header("ETag", STATIC_ETAG)
            self.end_headers()
            return

        with state_lock:
            state["stats"]["static_fetched"] += 1
        episodes = make_episode(1) + "\n" + make_episode(2)
        body = make_feed("Static Test Podcast", episodes)

        self.send_response(200)
        self.send_header("Content-Type", "application/rss+xml")
        self.send_header("ETag", STATIC_ETAG)
        self.end_headers()
        self.wfile.write(body.encode())

    def handle_dynamic(self):
        with state_lock:
            state["stats"]["dynamic_fetched"] += 1
            n = state["dynamic_episode_count"]
            # Increment for next request
            state["dynamic_episode_count"] += 1
        episodes = "\n".join(make_episode(i, "dynamic") for i in range(1, n + 1))
        body = make_feed("Dynamic Test Podcast", episodes)

        self.send_response(200)
        self.send_header("Content-Type", "application/rss+xml")
        self.end_headers()
        self.wfile.write(body.encode())

    def handle_stats(self):
        with state_lock:
            data = {**state["stats"], "dynamic_episode_count": state["dynamic_episode_count"]}
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def handle_reset(self):
        with state_lock:
            state["dynamic_episode_count"] = 1
            state["repaired_fixed"] = False
            state["stats"] = {"static_fetched": 0, "static_304": 0, "dynamic_fetched": 0}
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"status":"reset"}')


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8888
    server = ThreadingHTTPServer(("0.0.0.0", port), MockHandler)
    print(f"Mock RSS server on :{port}", flush=True)
    server.serve_forever()
