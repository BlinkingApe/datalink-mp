#!/usr/bin/env python3
"""PROTOTYPE, throwaway. Wipe me.  Run:  python3 prototype/turn-sync-activity/serve.py

Question (internet-play-speed 08): what does the page show while a Turn sync runs?
Serves the REAL crates/datalink-mp/src/page.html (fonts embedded) with a fake
/api/status that carries a scripted `turn_sync` object, plus the variants from
overlay.html injected on top. Nothing is persisted; nothing real is touched.
"""
import base64, json, re, sys, time, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
CRATE = HERE.parents[1] / "crates" / "datalink-mp" / "src"
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8765


# ---- the real page, with its fonts, plus the prototype overlay -------------
def faces():
    out = []
    for m in re.finditer(r'\("([^"]+)", (\d+), include_bytes!\("fonts/([^"]+)"\), (None|PI)\)',
                         (CRATE / "http.rs").read_text()):
        fam, wt, f, rng = m.groups()
        b64 = base64.b64encode((CRATE / "fonts" / f).read_bytes()).decode()
        ur = "unicode-range:U+2192,U+2713,U+03C0;" if rng == "PI" else ""
        out.append(f'@font-face{{font-family:"{fam}";font-weight:{wt};{ur}'
                   f'src:url(data:font/woff2;base64,{b64}) format("woff2");}}')
    return "\n".join(out)


def build_page():
    page = (CRATE / "page.html").read_text().replace("/* @font-face rules go here */", faces(), 1)
    ov = (HERE / "overlay.html").read_text()
    css, body, js = (re.search(rf"<!--{k}-->(.*?)<!--/{k}-->", ov, re.S).group(1) for k in ("css", "body", "js"))
    page = page.replace("</style>", css + "\n</style>", 1)
    page = page.replace("<main>", "<main>\n" + body, 1)
    page = page.replace("<script>", "<script>" + js + "</script>\n<script>", 1)
    return page


# ---- the fake Turn sync timeline (seconds on the script clock) -------------
# What the Helper could plausibly know from the JACKAL headers (kind, sequence,
# type, faction, turn) and from byte counts. Shaped like the captured Turn sync 1
# (24.7 s, 110 KB leaders resync, mostly compute), a little longer.
LOOP = 52.0
SYNC_START, SYNC_END = 4.0, 36.0   # 0x8301/0x4301 .. 0x4309
LINGER = 5.0                       # the Helper keeps "done" for a few seconds
# (t0, t1, phase, who the game waits on, bytes/s, note). who: 'friend' | 'you' | None (data moving)
SEGS = [
    (4.0, 5.0, 0, None, 1200, "turn ends: 0x8301 / 0x4301"),
    (5.0, 7.0, 1, None, 2000, "state syncs arrive"),
    (7.0, 17.0, 1, "friend", 0, "friend's game computing upkeep, nothing in flight"),
    (17.0, 18.0, 2, None, 1500, "barrier 0x2305 / 0x4305"),
    (18.0, 22.0, 3, "you", 0, "your game computing, nothing in flight"),
    (22.0, 30.0, 7, None, 13750, "RESYNC: leaders ~110 KB (0x4101 x68)"),
    (30.0, 32.0, 5, "friend", 0, "friend's game computing"),
    (32.0, 36.0, 6, None, 600, "barrier, then 0x4309 names who moves first"),
]
state = {"t0": time.time(), "paused": False, "pt": 0.0, "speed": 1.0, "conn": "direct", "turn": 17}
lock = threading.Lock()


def clock():
    with lock:
        t = state["pt"] if state["paused"] else (time.time() - state["t0"]) * state["speed"]
    return t % LOOP


def timeline(t):
    total, seg = 0.0, None
    for s in SEGS:
        a, b, _, _, bps, _ = s
        if t >= b:
            total += (b - a) * bps
        elif t >= a:
            total += (t - a) * bps
            seg = s
    if t < SYNC_START:
        return {"active": False, "just_finished": False, "raw_receiving": False, "t": t}
    if t >= SYNC_END:
        return {"active": False, "just_finished": t < SYNC_END + LINGER, "raw_receiving": False,
                "elapsed_ms": int((SYNC_END - SYNC_START) * 1000), "bytes": int(total),
                "turn": state["turn"], "next_mover": "friend", "t": t}
    a, b, ph, who, bps, note = seg
    return {"active": True, "just_finished": False,
            "elapsed_ms": int((t - SYNC_START) * 1000),
            "bytes": int(total),            # bytes received from this friend since the sync began
            "rate_bps": int(bps),           # over the last second
            "in_flight": bps > 0,           # something on the wire right now
            "raw_receiving": bps > 0,       # what a raw burst detector would say
            "phase": ph, "resync": ph == 7,
            "turn": state["turn"],
            "computing": who,               # which side's game the sync waits on (inferred: who sends next)
            "note": note, "t": t}


def status():
    return {
        "ticket": "smac" + "x7qk2" * 12, "ticket_seq": 1, "state": "hosting", "game_connected": True,
        "peers": ["f3a9c2"], "banners": [], "invalid_ticket": None, "os": "linux", "ipc_port": 9999,
        "self_check": {"passed": True, "game_exe": "terranx.exe", "folder": "/games/smac", "dll_found": True},
        "release_version": "0.1.0", "build_id": "PROTO", "ipc_version": 1, "peer_protocol_version": 1,
        # ---- invented fields: what the Helper would have to report (touches ticket 07) ----
        "friends": [{"id": "f3a9c2", "connection": state["conn"], "rtt_ms": 58 if state["conn"] == "direct" else 146}],
        "turn_sync": timeline(clock()),
    }


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def send(self, code, body, ctype="application/json"):
        b = body if isinstance(body, bytes) else body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        p = self.path.split("?")[0]
        if p == "/":
            self.send(200, build_page(), "text/html; charset=utf-8")  # re-read every load
        elif p == "/api/status":
            self.send(200, json.dumps(status()))
        else:
            self.send(404, "{}")

    def do_POST(self):
        p = self.path.split("?")[0]
        n = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(n) or b"{}")
        with lock:
            if p == "/proto/pause":
                if not state["paused"]:
                    state["pt"] = (time.time() - state["t0"]) * state["speed"]
                else:
                    state["t0"] = time.time() - state["pt"] / state["speed"]
                state["paused"] = not state["paused"]
            elif p == "/proto/restart":
                state.update(paused=False, t0=time.time() - body.get("at", SYNC_START - 2) / state["speed"])
            elif p == "/proto/speed":
                t = (time.time() - state["t0"]) * state["speed"]
                state["speed"] = float(body["v"])
                state["t0"] = time.time() - t / state["speed"]
            elif p == "/proto/conn":
                state["conn"] = body["v"]
        self.send(200, "{}")


if __name__ == "__main__":
    print(f"PROTOTYPE turn-sync-activity: http://127.0.0.1:{PORT}/?variant=A  (switch: bar, arrow keys, or A-E)")
    ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
