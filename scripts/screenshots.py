#!/usr/bin/env python3
"""Creates the README screenshots (docs/screenshots/*.png, 1440 × 900).

  python3 scripts/screenshots.py            # uses target/release/schnittwerk
  python3 scripts/screenshots.py --build    # builds it first (npx tauri build --no-bundle)
  python3 scripts/screenshots.py --only tracen,schneiden

Starts its own instance of the app with the WebKit inspector, drives it
through JavaScript and captures only that window (ImageMagick `import`).
The demo motifs live in scripts/screenshot-motifs/.

The app shares its settings (localStorage) with an installed Schnittwerk:
the script saves them before and writes them back afterwards, and removes
the autosave copy it creates. Linux/X11 only.

Needs: python3-websockets, ImageMagick, xwininfo/xprop (x11-utils), xrandr.
"""
import argparse, asyncio, ctypes, json, os, re, signal, subprocess, sys, tempfile, time
from pathlib import Path

import websockets

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target/release/schnittwerk"
OUT = ROOT / "docs/screenshots"
MOTIFS = ROOT / "scripts/screenshot-motifs"
AUTOSAVE = Path.home() / ".local/share/de.schnittwerk.desktop/autosave.schnitt"
PORT = 9222
URL = f"ws://127.0.0.1:{PORT}/socket/1/1/WebPage"
W, H = 1440, 900

# ---------------------------------------------------------------------------
# Talking to the page


class Page:
    def __init__(self):
        self.ws = None
        self.target = None
        self.n = 0

    async def connect(self):
        for _ in range(60):
            try:
                self.ws = await websockets.connect(URL, max_size=None)
                break
            except OSError:
                await asyncio.sleep(0.5)
        else:
            raise SystemExit("Inspector nicht erreichbar")
        while self.target is None:
            msg = json.loads(await asyncio.wait_for(self.ws.recv(), 10))
            if msg.get("method") == "Target.targetCreated":
                self.target = msg["params"]["targetInfo"]["targetId"]

    async def js(self, expr: str):
        """Evaluates an expression (sync) and returns its JSON value."""
        self.n += 1
        inner = {"id": self.n, "method": "Runtime.evaluate", "params": {"expression": expr, "returnByValue": True}}
        await self.ws.send(json.dumps({"id": self.n, "method": "Target.sendMessageToTarget", "params": {"targetId": self.target, "message": json.dumps(inner)}}))
        while True:
            msg = json.loads(await asyncio.wait_for(self.ws.recv(), 20))
            if msg.get("method") != "Target.dispatchMessageFromTarget":
                continue
            m = json.loads(msg["params"]["message"])
            if m.get("id") != self.n:
                continue
            r = m.get("result", {})
            if r.get("wasThrown"):
                raise RuntimeError(f"JS-Fehler in: {expr[:120]} → {r.get('result')}")
            return r.get("result", {}).get("value")


# Helpers installed in the page (see the README section on screenshots).
HELPERS = r"""
window.__q = (sel, text) => [...document.querySelectorAll(sel)].find((e) => text === undefined || e.textContent.trim() === text || e.textContent.includes(text));
window.__click = (sel, text) => { const e = __q(sel, text); if (!e) throw new Error("nicht gefunden: " + sel + " " + (text ?? "")); e.click(); return true; };
window.__key = (key, ctrl = false) => { window.dispatchEvent(new KeyboardEvent("keydown", { key, ctrlKey: ctrl, bubbles: true })); return true; };
window.__set = (el, value, ev = "change") => { el.value = value; el.dispatchEvent(new Event(ev, { bubbles: true })); return true; };
window.__mat = () => { const g = document.querySelector(".stage > .wrap > svg > g[transform]"); return g.querySelectorAll(":scope > rect")[2].getBoundingClientRect(); };
window.__mm = (x, y, W = 203.2, H = 304.8) => { const r = __mat(); return { x: r.left + x * r.width / W, y: r.top + y * r.height / H }; };
window.__ev = (type, p, el) => el.dispatchEvent(new PointerEvent(type, { clientX: p.x, clientY: p.y, bubbles: true, cancelable: true, pointerId: 1, pointerType: "mouse", button: 0, buttons: type === "pointerup" ? 0 : 1, isPrimary: true }));
window.__tool = (label) => [...document.querySelectorAll(".rail .tool")].find((b) => b.title.startsWith(label)).click();
window.__draw = (label, x0, y0, x1, y1) => { __tool(label); const sv = document.querySelector(".stage > .wrap > svg"); setTimeout(() => { const a = __mm(x0, y0), b = __mm(x1, y1); __ev("pointerdown", a, sv); __ev("pointermove", { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }, sv); __ev("pointermove", b, sv); __ev("pointerup", b, sv); }, 80); return true; };
window.__clear = () => { __key("a", true); __key("Delete"); return true; };
window.__surfaceOpen = () => { if (!document.querySelector(".chip-wrap .pop")) document.querySelector(".chip-wrap .chip").click(); return true; };
window.__surfacePick = (label) => { const it = [...document.querySelectorAll(".chip-wrap .pop .item")].find((b) => b.textContent.trim() === label || (label.length > 3 && b.textContent.includes(label))); if (!it) throw new Error("Fläche nicht gefunden: " + label); it.click(); return true; };
window.__closePopups = () => { document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })); return true; };
// File dialogs: answered by the script instead of a real GTK dialog.
window.__pick = [];
if (!window.__fetchPatched) {
  const of = window.fetch;
  window.fetch = async (u, o) => String(u).includes("plugin%3Adialog%7Copen")
    ? new Response(JSON.stringify(window.__pick), { headers: { "Tauri-Response": "ok", "Content-Type": "application/json" } })
    : of(u, o);
  window.__fetchPatched = true;
}
window.__import = (paths) => { window.__pick = paths; document.querySelector("[data-tour=import]").click(); return true; };
window.__layersOpen = () => { if (!document.querySelector(".layers-body")) document.querySelector(".layers-head").click(); return true; };
window.__layer = (name) => { const b = [...document.querySelectorAll(".layers-body li .main")].find((b) => b.textContent.trim() === name); if (!b) throw new Error("Ebene nicht gefunden: " + name); b.click(); return true; };
window.__slider = (label, v) => { const l = [...document.querySelectorAll("aside .slider")].find((l) => l.textContent.includes(label)); return __set(l.querySelector("input"), v, "input"); };
window.__step = (i) => { document.querySelectorAll(".steps .step")[i].click(); return true; };
window.__previewClick = (x, y) => { const sv = document.querySelector(".preview svg"); const p = new DOMPoint(x, y).matrixTransform(sv.getScreenCTM()); sv.dispatchEvent(new PointerEvent("pointerdown", { clientX: p.x, clientY: p.y, bubbles: true, pointerId: 1, pointerType: "mouse", buttons: 1 })); return true; };
true;
"""

# ---------------------------------------------------------------------------
# Window handling (X11)


def run(*cmd) -> str:
    return subprocess.run(cmd, capture_output=True, text=True).stdout


def window_of(pid: int) -> str:
    for line in run("xwininfo", "-root", "-tree").splitlines():
        if '"Schnittwerk"' not in line and '"schnittwerk"' not in line:
            continue
        wid = line.split()[0]
        if run("xprop", "-id", wid, "_NET_WM_PID").strip().endswith(f" {pid}") and "IsViewable" in run("xwininfo", "-id", wid):
            return wid
    raise SystemExit("Fenster nicht gefunden")


def landscape_monitor() -> tuple[int, int]:
    """Top-left corner of a monitor that is at least W × H and wider than high."""
    for line in run("xrandr", "--listmonitors").splitlines()[1:]:
        m = re.search(r"(\d+)/\d+x(\d+)/\d+\+(\d+)\+(\d+)", line)
        if m:
            w, h, x, y = map(int, m.groups())
            if w >= W and h >= H and w > h:
                return x + (w - W) // 2, y + max(0, (h - H) // 2 - 20)
    return 0, 0


def resize(wid: str, x: int, y: int):
    """Unmaximizes the window (_NET_WM_STATE) and moves/resizes it."""
    x11 = ctypes.cdll.LoadLibrary("libX11.so.6")
    c_ulong, c_long, c_int, c_void_p = ctypes.c_ulong, ctypes.c_long, ctypes.c_int, ctypes.c_void_p

    class ClientMessage(ctypes.Structure):
        _fields_ = [("type", c_int), ("serial", c_ulong), ("send_event", c_int), ("display", c_void_p),
                    ("window", c_ulong), ("message_type", c_ulong), ("format", c_int), ("data", c_long * 5)]

    class Event(ctypes.Union):
        _fields_ = [("xclient", ClientMessage), ("pad", c_long * 24)]

    x11.XOpenDisplay.restype = c_void_p
    x11.XInternAtom.restype = c_ulong
    x11.XInternAtom.argtypes = [c_void_p, ctypes.c_char_p, c_int]
    x11.XDefaultRootWindow.restype = c_ulong
    x11.XDefaultRootWindow.argtypes = [c_void_p]
    x11.XSendEvent.argtypes = [c_void_p, c_ulong, c_int, c_long, ctypes.POINTER(Event)]
    x11.XMoveResizeWindow.argtypes = [c_void_p, c_ulong, c_int, c_int, ctypes.c_uint, ctypes.c_uint]
    x11.XSync.argtypes = [c_void_p, c_int]
    d = x11.XOpenDisplay(None)
    ev = Event()
    ev.xclient.type = 33  # ClientMessage
    ev.xclient.send_event = 1
    ev.xclient.window = int(wid, 16)
    ev.xclient.message_type = x11.XInternAtom(d, b"_NET_WM_STATE", 0)
    ev.xclient.format = 32
    ev.xclient.data[0] = 0  # remove
    ev.xclient.data[1] = x11.XInternAtom(d, b"_NET_WM_STATE_MAXIMIZED_HORZ", 0)
    ev.xclient.data[2] = x11.XInternAtom(d, b"_NET_WM_STATE_MAXIMIZED_VERT", 0)
    ev.xclient.data[3] = 1
    x11.XSendEvent(d, x11.XDefaultRootWindow(d), 0, (1 << 20) | (1 << 19), ctypes.byref(ev))
    x11.XSync(d, 0)
    time.sleep(0.3)
    x11.XMoveResizeWindow(d, int(wid, 16), x, y, W, H)
    x11.XSync(d, 0)


# ---------------------------------------------------------------------------
# Scenes


def motif(name: str) -> str:
    return str(MOTIFS / f"{name}.png")


async def tick(p: Page, s: float = 0.3):
    """Svelte updates the page after a click with a short delay."""
    await asyncio.sleep(s)


async def wait(p: Page, cond: str, timeout: float = 15):
    """Waits until the JS condition is true (e.g. an element appeared)."""
    end = time.time() + timeout
    while time.time() < end:
        if await p.js(f"!!({cond})"):
            await tick(p, 0.2)
            return
        await asyncio.sleep(0.25)
    raise RuntimeError(f"Zeitüberschreitung beim Warten auf: {cond}")


async def surface(p: Page, label: str):
    await p.js("__surfaceOpen()")
    await tick(p)
    await p.js(f"__surfacePick({json.dumps(label)})")
    await tick(p)
    await p.js("__closePopups()")
    await tick(p)


async def layer(p: Page, name: str):
    await p.js("__layersOpen()")
    await tick(p)
    await p.js(f"__layer({json.dumps(name)})")
    await tick(p, 0.4)


async def scene_startseite(p: Page):
    await p.js('__click("[data-tour=home]")')
    await asyncio.sleep(0.8)


async def close_start(p: Page):
    await p.js('__q("button", "Leere Matte") ? __click("button", "Leere Matte") : true')
    await asyncio.sleep(0.4)


async def scene_tracen(p: Page):
    await close_start(p)
    await p.js("__clear()")
    await surface(p, "Portrait")
    await p.js(f'__import({json.dumps([motif("berge")])})')
    await wait(p, '__q("aside .way", "In Schnittlinien umwandeln")')
    await p.js('__click("aside .way", "In Schnittlinien umwandeln")')
    await wait(p, '__q("aside .stats", "Konturen")')


async def scene_formen(p: Page):
    await close_start(p)
    await p.js("__clear()")
    await surface(p, "Portrait")
    await p.js('__key("0", true)')
    for args in ['"Herz", 60, 120, 145, 200', '"Stern", 20, 210, 80, 270', '"Vieleck", 125, 215, 185, 270', '"Rechteck", 85, 225, 120, 290']:
        await p.js(f"__draw({args})")
        await asyncio.sleep(0.5)
    await p.js('(() => { __tool("Text"); setTimeout(() => { const sv = document.querySelector(".stage > .wrap > svg"); const q = __mm(50, 40); __ev("pointerdown", q, sv); __ev("pointerup", q, sv); }, 80); return true; })()')
    await wait(p, 'document.querySelector("textarea.area")')
    await p.js('__set(document.querySelector("textarea.area"), "Schnittwerk", "input")')
    await p.js('(() => { const f = [...document.querySelectorAll("aside input.field")].find((i) => i.placeholder?.startsWith("Schrift")); return __set(f, "DejaVu Sans", "input"); })()')
    await wait(p, '[...document.querySelectorAll(".fontpick select option")].some((o) => o.value.startsWith("DejaVu Sans"))')
    await p.js('(() => { const s = document.querySelector(".fontpick select"); const v = [...s.options].map((o) => o.value).find((v) => v.startsWith("DejaVu Sans")) ?? s.options[0].value; return __set(s, v); })()')
    await asyncio.sleep(0.5)
    await p.js('(() => { __set(document.querySelector(".num input[type=number][min=\\"1\\"]"), 24); document.querySelectorAll("aside .btn.style")[0].click(); return true; })()')
    await asyncio.sleep(0.6)
    await p.js('__click("aside .seg button", "Bogen ⌒")')
    await wait(p, '__q("aside .slider", "Radius")')
    await p.js('__slider("Radius", 75)')
    await asyncio.sleep(0.8)
    w = await p.js('parseFloat(document.querySelectorAll("aside .grid .num input")[2].value.replace(",", "."))')
    await p.js(f'(() => {{ const f = document.querySelectorAll("aside .grid .num input"); __set(f[0], "{(203.2 - w) / 2:.1f}".replace(".", ",")); __set(f[1], "58"); return true; }})()')
    await asyncio.sleep(0.4)
    await layer(p, "Rechteck")
    await p.js('__slider("Eckenradius", 6)')
    await layer(p, "Stern")
    await p.js('document.querySelector(".layers-head").click()')
    await asyncio.sleep(0.6)


async def scene_sticker_assistent(p: Page):
    await close_start(p)
    await p.js("__clear()")
    await surface(p, "A4")
    await p.js(f'__import({json.dumps([motif("sonne")])})')
    await wait(p, '__q("aside .seg button", "Transparenz")')
    await p.js('__click("aside .seg button", "Transparenz")')
    await asyncio.sleep(0.5)
    await wait(p, '__q("aside .stats", "Kontur")')


async def scene_sticker_bogen(p: Page):
    await close_start(p)
    await p.js("__clear()")
    await surface(p, "A4")
    await p.js(f'__import({json.dumps([motif("sonne"), motif("kaktus"), motif("erdbeere")])})')
    await wait(p, 'document.querySelectorAll(".layers-head .count, aside .layers-head").length && /\\b3\\b/.test(document.querySelector(".layers-head").textContent)')
    await p.js('__key("a", true)')
    await asyncio.sleep(0.4)
    await p.js('document.querySelector("aside .back") ? (document.querySelector("aside .back").click(), true) : true')
    await asyncio.sleep(0.4)
    await p.js('__set([...document.querySelectorAll("aside .grid.multi .num")].find((l) => l.title === "Höhe jedes Objekts").querySelector("input"), "48")')
    await asyncio.sleep(0.4)
    await p.js('__click("aside .way", "Sticker-Bogen")')
    await wait(p, '__q("aside .slider", "Exemplare")')
    await p.js('(() => { __click("aside .seg button", "Transparenz"); return __slider("Exemplare", 4); })()')
    await asyncio.sleep(0.3)
    await p.js('__click("aside button", "Sticker-Bogen erstellen")')
    for _ in range(40):
        await asyncio.sleep(0.5)
        if await p.js('!!document.querySelector(".next")'):
            break
    await p.js('__key("Escape")')
    await asyncio.sleep(0.6)


async def scene_schneiden(p: Page):
    # Continues on the sticker sheet.
    await p.js('__click("[data-tour=cut]")')
    await wait(p, 'document.querySelector(".steps")')
    await p.js('__step(1)')
    await wait(p, 'document.querySelector(".step-body section .row select")')
    await p.js('(() => { document.querySelectorAll(".ctab")[0]?.click(); return __set(document.querySelector(".step-body section .row select"), "sticker-matt"); })()')
    await asyncio.sleep(0.3)
    await p.js('__step(2)')
    await asyncio.sleep(0.4)
    await p.js('(() => { const s = document.querySelector(".step-body .select-row select"); if (s) { s.selectedIndex = 0; s.dispatchEvent(new Event("change", { bubbles: true })); } return __previewClick(70, 240); })()')
    await asyncio.sleep(0.6)


async def scene_kacheln(p: Page):
    await p.js('document.querySelector(".dialog header .btn.icon") ? (document.querySelector(".dialog header .btn.icon").click(), true) : true')
    await asyncio.sleep(0.4)
    await p.js("__clear()")
    await p.js("__surfaceOpen()")
    await tick(p)
    await p.js('__surfacePick("Eigene Größe")')
    await tick(p)
    await p.js('__set(document.querySelectorAll(".chip-wrap .pop .size input")[0], 420)')
    await tick(p)
    await p.js('__set(document.querySelectorAll(".chip-wrap .pop .size input")[1], 420)')
    await tick(p)
    await p.js("__closePopups()")
    await tick(p)
    await p.js(f'__import({json.dumps([motif("berge")])})')
    await wait(p, '__q("aside .way", "In Schnittlinien umwandeln")')
    await p.js('__click("aside .way", "In Schnittlinien umwandeln")')
    await wait(p, '__q("aside .stats", "Konturen")')
    await p.js('__click("aside .btn.primary", "Übernehmen")')
    await wait(p, 'document.querySelectorAll("aside .grid .num input").length >= 3')
    await p.js('__set(document.querySelectorAll("aside .grid .num input")[2], "380")')
    await asyncio.sleep(0.3)
    await p.js('(() => { document.querySelector("aside [title=\\"Horizontal zentrieren\\"]").click(); document.querySelector("aside [title=\\"Vertikal zentrieren\\"]").click(); return __key("0", true); })()')
    await asyncio.sleep(0.4)
    await p.js('__click("[data-tour=cut]")')
    await wait(p, 'document.querySelector(".steps")')
    await p.js('__step(1)')
    await wait(p, 'document.querySelector(".step-body section .row select")')
    await p.js('__set(document.querySelector(".step-body section .row select"), "vinyl")')
    await tick(p)
    await p.js('__step(3)')
    await asyncio.sleep(0.8)


# Order matters: "schneiden" continues on the sheet from "sticker-bogen".
SCENES = [
    ("startseite", scene_startseite),
    ("tracen", scene_tracen),
    ("formen", scene_formen),
    ("sticker-assistent", scene_sticker_assistent),
    ("sticker-bogen", scene_sticker_bogen),
    ("schneiden", scene_schneiden),
    ("kacheln", scene_kacheln),
]

# ---------------------------------------------------------------------------


async def main(only: set[str] | None):
    if subprocess.run(["curl", "-s", f"127.0.0.1:{PORT}"], capture_output=True).returncode == 0:
        raise SystemExit(f"Port {PORT} ist belegt – läuft schon eine Test-Instanz?")
    started = time.time()
    autosave_before = AUTOSAVE.stat().st_mtime if AUTOSAVE.exists() else None
    env = dict(os.environ, GDK_BACKEND="x11", WEBKIT_INSPECTOR_HTTP_SERVER=f"127.0.0.1:{PORT}")
    proc = subprocess.Popen([str(BIN)], cwd=ROOT, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    p = Page()
    backup = None
    try:
        await p.connect()
        await asyncio.sleep(3)
        backup = await p.js("JSON.stringify(Object.fromEntries(Object.keys(localStorage).map((k) => [k, localStorage.getItem(k)])))")
        wid = window_of(proc.pid)
        resize(wid, *landscape_monitor())
        await asyncio.sleep(1)
        size = await p.js('innerWidth + "x" + innerHeight')
        if size != f"{W}x{H}":
            print(f"Warnung: Fenstergröße {size} statt {W}x{H}")
        await p.js(HELPERS)
        # Messages (toasts) would end up in the pictures.
        await p.js('(() => { const st = document.createElement("style"); st.textContent = ".toasts { display: none !important; }"; document.head.append(st); return true; })()')
        # No tour, centred mat, start page closed and reopened via the logo.
        await p.js('localStorage.setItem("schnittwerk.tourDone", "1"); true')
        await p.js('__q("button", "Leere Matte") ? __click("button", "Leere Matte") : true')
        await asyncio.sleep(0.5)
        await p.js('(() => { __q("button", "Überspringen")?.click(); return __key("0", true); })()')
        await asyncio.sleep(0.4)
        OUT.mkdir(parents=True, exist_ok=True)
        for name, scene in SCENES:
            try:
                await scene(p)
            except Exception:
                shot = Path(tempfile.gettempdir()) / f"schnittwerk-screenshot-fehler-{name}.png"
                subprocess.run(["import", "-window", wid, str(shot)])
                print(f"Fehler in Szene „{name}“ – Bild des Fensters: {shot}")
                raise
            if only is None or name in only:
                subprocess.run(["import", "-window", wid, str(OUT / f"{name}.png")], check=True)
                print("✓", name)
    finally:
        try:
            if backup is not None:
                await p.js(
                    "(() => { const b = " + json.dumps(json.loads(backup)) + "; "
                    "for (const k of Object.keys(localStorage)) if (!(k in b)) localStorage.removeItem(k); "
                    "for (const [k, v] of Object.entries(b)) localStorage.setItem(k, v); return true; })()"
                )
                after = await p.js("JSON.stringify(Object.fromEntries(Object.keys(localStorage).map((k) => [k, localStorage.getItem(k)])))")
                print("Einstellungen wiederhergestellt" if json.loads(after) == json.loads(backup) else "WARNUNG: Einstellungen weichen ab!")
        finally:
            proc.send_signal(signal.SIGTERM)
            proc.wait(timeout=10)
            # Remove the autosave copy only if this run created or changed it.
            if AUTOSAVE.exists() and AUTOSAVE.stat().st_mtime >= started and AUTOSAVE.stat().st_mtime != autosave_before:
                AUTOSAVE.unlink()


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--build", action="store_true", help="vorher die Release-Version bauen")
    ap.add_argument("--only", help="nur diese Bilder speichern, z. B. tracen,schneiden")
    args = ap.parse_args()
    if args.build:
        subprocess.run(["npx", "tauri", "build", "--no-bundle"], cwd=ROOT, check=True)
    if not BIN.exists():
        sys.exit(f"{BIN} fehlt – mit --build bauen")
    asyncio.run(main(set(args.only.split(",")) if args.only else None))
