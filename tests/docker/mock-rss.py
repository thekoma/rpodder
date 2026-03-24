#!/usr/bin/env python3
"""
Mock RSS server for rpodder integration tests.

Endpoints:
  /feed/static   — Always returns the same 2-episode RSS feed.
                   Supports ETag: returns 304 on second request.
  /feed/dynamic  — Returns a feed with N+1 episodes each request
                   (starts with 1, then 2, then 3...).
  /stats         — JSON counters: how many times each feed was fetched
                   and how many 304s were returned.
  /reset         — Reset counters and dynamic state.

Usage:
  python3 mock-rss.py [port]     (default: 8888)
"""

import json
import hashlib
from http.server import HTTPServer, BaseHTTPRequestHandler
import sys

# Global state
state = {
    "dynamic_episode_count": 1,
    "stats": {
        "static_fetched": 0,
        "static_304": 0,
        "dynamic_fetched": 0,
    },
}

STATIC_ETAG = '"static-feed-v1"'


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


class MockHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        # Suppress default logging
        pass

    def do_GET(self):
        if self.path == "/feed/static":
            self.handle_static()
        elif self.path == "/feed/dynamic":
            self.handle_dynamic()
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

    def handle_static(self):
        # Check If-None-Match for conditional GET
        if_none_match = self.headers.get("If-None-Match", "")
        if if_none_match == STATIC_ETAG:
            state["stats"]["static_304"] += 1
            self.send_response(304)
            self.send_header("ETag", STATIC_ETAG)
            self.end_headers()
            return

        state["stats"]["static_fetched"] += 1
        episodes = make_episode(1) + "\n" + make_episode(2)
        body = make_feed("Static Test Podcast", episodes)

        self.send_response(200)
        self.send_header("Content-Type", "application/rss+xml")
        self.send_header("ETag", STATIC_ETAG)
        self.end_headers()
        self.wfile.write(body.encode())

    def handle_dynamic(self):
        state["stats"]["dynamic_fetched"] += 1
        n = state["dynamic_episode_count"]
        episodes = "\n".join(make_episode(i, "dynamic") for i in range(1, n + 1))
        body = make_feed("Dynamic Test Podcast", episodes)

        # Increment for next request
        state["dynamic_episode_count"] += 1

        self.send_response(200)
        self.send_header("Content-Type", "application/rss+xml")
        self.end_headers()
        self.wfile.write(body.encode())

    def handle_stats(self):
        data = {**state["stats"], "dynamic_episode_count": state["dynamic_episode_count"]}
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def handle_reset(self):
        state["dynamic_episode_count"] = 1
        state["stats"] = {"static_fetched": 0, "static_304": 0, "dynamic_fetched": 0}
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"status":"reset"}')


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8888
    server = HTTPServer(("0.0.0.0", port), MockHandler)
    print(f"Mock RSS server on :{port}", flush=True)
    server.serve_forever()
