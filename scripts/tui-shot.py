#!/usr/bin/env python3
"""Render a captured TUI frame as a README screenshot.

Reads the ANSI text `tmux capture-pane -e` produces, swaps the capturing
machine's app roster for a neutral demo one, and renders the result to PNG
through headless Chrome.

The substitution is the point: the layout, colours, column widths, borders and
selection highlight are all genuinely the TUI's output, but the app names and
bundle IDs are replaced so a screenshot never advertises what is installed on
the machine that took it. Replacements are width-compensated, so every column
stays aligned exactly where the TUI put it.

Usage:  tui-shot.py <frame.ansi> <out.png>
"""

import html
import os
import re
import subprocess
import sys
import tempfile

# --------------------------------------------------------------------------
# Demo roster
# --------------------------------------------------------------------------
# Real name -> (demo name, demo bundle ID). Entries mapping a name to itself
# are the ones deliberately kept: Apple's stock apps and a few ubiquitous
# third-party ones say nothing about the machine, and the demo roster is the
# same one the GUI screenshots use (Chrome, IINA, Numbers, Preview, TextEdit,
# Typora, Visual Studio Code, Xcode).
#
# Substitutes are chosen to keep the list alphabetically sorted -- the TUI
# sorts its panes, so a replacement has to land in the same slot to avoid a
# visibly out-of-order list.
ROSTER = {
    # --- kept as-is -------------------------------------------------------
    "AirPort Utility": ("AirPort Utility", "com.apple.airport.airportutility"),
    "App Store": ("App Store", "com.apple.AppStore"),
    "Automator": ("Automator", "com.apple.Automator"),
    "Books": ("Books", "com.apple.iBooksX"),
    "Calendar": ("Calendar", "com.apple.iCal"),
    "calibre": ("calibre", "net.kovidgoyal.calibre"),
    "ChatGPT": ("ChatGPT", "com.openai.chat"),
    "Chess": ("Chess", "com.apple.Chess"),
    "Claude": ("Claude", "com.anthropic.claudefordesktop"),
    "ColorSync Utility": ("ColorSync Utility", "com.apple.ColorSyncUtility"),
    "Console": ("Console", "com.apple.Console"),
    "Contacts": ("Contacts", "com.apple.AddressBook"),
    "Font Book": ("Font Book", "com.apple.FontBook"),
    "Ghostty": ("Ghostty", "com.mitchellh.ghostty"),
    "Google Chrome": ("Google Chrome", "com.google.Chrome"),
    "Grapher": ("Grapher", "com.apple.grapher"),
    "IINA": ("IINA", "com.colliderli.iina"),
    "Keynote": ("Keynote", "com.apple.iWork.Keynote"),
    "Magnifier": ("Magnifier", "com.apple.Magnifier"),
    "Messages": ("Messages", "com.apple.MobileSMS"),
    "Music": ("Music", "com.apple.Music"),
    "Notes": ("Notes", "com.apple.Notes"),
    "Numbers": ("Numbers", "com.apple.Numbers"),
    "Pages": ("Pages", "com.apple.iWork.Pages"),
    "Photos": ("Photos", "com.apple.Photos"),
    "Preview": ("Preview", "com.apple.Preview"),
    "QuickTime Player": ("QuickTime Player", "com.apple.QuickTimePlayerX"),
    "Safari": ("Safari", "com.apple.Safari"),
    "Screen Sharing": ("Screen Sharing", "com.apple.ScreenSharing"),
    "Script Editor": ("Script Editor", "com.apple.ScriptEditor2"),
    "Slack": ("Slack", "com.tinyspeck.slackmacgap"),
    "System Information": ("System Information", "com.apple.SystemProfiler"),
    "System Settings": ("System Settings", "com.apple.systempreferences"),
    "Terminal": ("Terminal", "com.apple.Terminal"),
    "TextEdit": ("TextEdit", "com.apple.TextEdit"),
    "Tips": ("Tips", "com.apple.tips"),
    "TV": ("TV", "com.apple.TV"),
    "Typora": ("Typora", "abnerworks.Typora"),
    "Zed": ("Zed", "dev.zed.Zed"),
    "Zotero": ("Zotero", "org.zotero.zotero"),
    # --- substituted ------------------------------------------------------
    "Bob": ("Bear", "net.shinyfrog.bear"),
    "Camera RawX": ("Canva", "com.canva.CanvaDesktop"),
    "CleanShot X": ("Cloudflare WARP", "com.cloudflare.1dot1dot1dot1"),
    "CotEditor": ("Cursor", "com.todesktop.cursor"),
    "DBX": ("Discord", "com.hnc.Discord"),
    "Downie 4": ("Docker", "com.docker.docker"),
    "Eudic": ("Emacs", "org.gnu.Emacs"),
    "FUJIFILM X RAW STUDIO": ("GarageBand", "com.apple.garageband10"),
    "Goodnotes": ("GoLand", "com.jetbrains.goland"),
    "iCost": ("iA Writer", "pro.writer.mac"),
    "KeyboardHolder": ("Keka", "com.aone.keka"),
    "MacWhisper": ("Linear", "com.linear"),
    "Marked": ("Marked", "com.brettterpstra.marked2"),
    "MindNode": ("Notion", "notion.id"),
    "NeteaseMusic": ("Obsidian", "md.obsidian"),
    "Numi": ("OmniGraffle", "com.omnigroup.OmniGraffle7"),
    "Parallels Desktop": ("UTM", "com.utmapp.UTM"),
    "PDF Reader Pro": ("PDF Expert", "com.readdle.PDFExpert-Mac"),
    "Permute 4": ("HandBrake", "fr.handbrake.HandBrake"),
    "Picview": ("Postman", "com.postmanlabs.mac"),
    "PopClip": ("Proxyman", "com.proxyman.NSProxy"),
    "QSpace Pro": ("Raycast", "com.raycast.macos"),
    "Raycast": ("Rectangle", "com.knollsoft.Rectangle"),
    "Screen Studio": ("Sublime Text", "com.sublimetext.4"),
    "Surge": ("Sublime Merge", "com.sublimemerge"),
    "Surge Dashboard": ("Sublime Merge", "com.sublimemerge"),
    "哔哩哔哩": ("VLC", "org.videolan.vlc"),
}

# Bundle IDs are swapped independently of names: a row can show a bundle ID for
# an app whose name is off-screen, and Launch Services hands back raw IDs for
# bundles the scanner never saw.
REAL_BUNDLE_IDS = {
    "com.eusoft.eudic": "org.gnu.Emacs",
    "com.anogeissus.CameraRawX": "com.canva.CanvaDesktop",
    "net.kovidgoyal.calibre": "net.kovidgoyal.calibre",
    "com.bilibili.bilibiliPC": "org.videolan.vlc",
    "com.parallels.desktop.console": "com.utmapp.UTM",
    "com.coteditor.CotEditor": "com.todesktop.cursor",
    "com.charliemonroe.Downie-4": "com.docker.docker",
    "com.charliemonroe.Permute-3": "fr.handbrake.HandBrake",
    "com.ripperhe.Bob": "net.shinyfrog.bear",
    "com.netease.163music": "md.obsidian",
    "com.dbx.DBX": "com.hnc.Discord",
    "com.goodnotes.mac": "com.jetbrains.goland",
    "com.apple.iBooksX": "com.apple.iBooksX",
}

# --------------------------------------------------------------------------
# ANSI parsing
# --------------------------------------------------------------------------
SGR = re.compile(r"\x1b\[([0-9;]*)m")

# xterm-256 palette, enough of it for what ratatui emits here.
BASE16 = [
    "#1c1f26", "#e06c75", "#98c379", "#e5c07b",
    "#61afef", "#c678dd", "#56b6c2", "#abb2bf",
    "#5c6370", "#e06c75", "#98c379", "#e5c07b",
    "#61afef", "#c678dd", "#56b6c2", "#ffffff",
]
FG_DEFAULT = "#c8ccd4"
BG_DEFAULT = "#14171d"


def color256(n):
    if n < 16:
        return BASE16[n]
    if n < 232:
        n -= 16
        r, g, b = n // 36, (n % 36) // 6, n % 6
        scale = [0, 95, 135, 175, 215, 255]
        return "#%02x%02x%02x" % (scale[r], scale[g], scale[b])
    v = 8 + (n - 232) * 10
    return "#%02x%02x%02x" % (v, v, v)


class Style:
    def __init__(self):
        self.fg = None
        self.bg = None
        self.bold = False

    def copy(self):
        s = Style()
        s.fg, s.bg, s.bold = self.fg, self.bg, self.bold
        return s

    def apply(self, params):
        codes = [int(p) if p else 0 for p in params.split(";")] or [0]
        i = 0
        while i < len(codes):
            c = codes[i]
            if c == 0:
                self.fg = self.bg = None
                self.bold = False
            elif c == 1:
                self.bold = True
            elif c == 22:
                self.bold = False
            elif c == 39:
                self.fg = None
            elif c == 49:
                self.bg = None
            elif 30 <= c <= 37:
                self.fg = BASE16[c - 30]
            elif 90 <= c <= 97:
                self.fg = BASE16[c - 90 + 8]
            elif 40 <= c <= 47:
                self.bg = BASE16[c - 40]
            elif c in (38, 48) and i + 2 < len(codes) and codes[i + 1] == 5:
                col = color256(codes[i + 2])
                if c == 38:
                    self.fg = col
                else:
                    self.bg = col
                i += 2
            i += 1

    def css(self):
        bits = []
        if self.fg:
            bits.append(f"color:{self.fg}")
        if self.bg:
            bits.append(f"background:{self.bg}")
        if self.bold:
            bits.append("font-weight:600")
        return ";".join(bits)


def split_segments(line):
    """-> [(style, text)] with the escape codes resolved into styles."""
    out = []
    style = Style()
    pos = 0
    for m in SGR.finditer(line):
        if m.start() > pos:
            out.append((style.copy(), line[pos:m.start()]))
        style.apply(m.group(1))
        pos = m.end()
    if pos < len(line):
        out.append((style.copy(), line[pos:]))
    return out


# --------------------------------------------------------------------------
# Roster substitution
# --------------------------------------------------------------------------
def substitute(visible):
    """Swap app names and bundle IDs, preserving every column's width.

    Each replacement absorbs its length difference into the run of spaces that
    immediately follows the token -- both the name and the bundle ID column are
    always padded before the next column or the pane border, so the line width
    and therefore the whole layout is unchanged.
    """
    pairs = [(real, demo) for real, (demo, _) in ROSTER.items() if real != demo]
    pairs += [(rid, did) for rid, did in REAL_BUNDLE_IDS.items() if rid != did]
    # Longest first, so "Surge Dashboard" is not eaten by "Surge".
    pairs.sort(key=lambda p: -len(p[0]))

    for real, demo in pairs:
        start = 0
        while True:
            idx = visible.find(real, start)
            if idx < 0:
                break
            end = idx + len(real)
            # Only substitute whole words -- "Music" must not fire inside
            # "NeteaseMusic" (which has its own entry, handled by length order).
            before_ok = idx == 0 or not visible[idx - 1].isalnum()
            after_ok = end >= len(visible) or not visible[end].isalnum()
            if not (before_ok and after_ok):
                start = end
                continue

            pad = len(visible[end:]) - len(visible[end:].lstrip(" "))
            delta = len(demo) - len(real)
            if delta > 0 and pad < delta:
                # Not enough padding to absorb a longer name: truncate instead
                # of pushing the column out of alignment.
                demo_fit = demo[: len(real) + pad]
                visible = visible[:idx] + demo_fit + visible[end + pad:]
                start = idx + len(demo_fit)
                continue

            visible = visible[:idx] + demo + " " * (pad - delta) + visible[end + pad:]
            start = idx + len(demo)
    return visible


def sanitize_line(line):
    segs = split_segments(line)
    rebuilt = []
    for style, text in segs:
        rebuilt.append((style, substitute(text)))
    return rebuilt


# --------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------
PAGE = """<!doctype html>
<html><head><meta charset="utf-8"><style>
  * {{ box-sizing: border-box; }}
  html, body {{ margin: 0; padding: 0; background: {page_bg}; }}
  body {{ padding: 40px; display: inline-block; }}
  .window {{
    background: {term_bg};
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 1px 1px rgba(0,0,0,.10), 0 8px 28px rgba(0,0,0,.22);
  }}
  .titlebar {{
    height: 34px; display: flex; align-items: center; gap: 8px;
    padding: 0 14px; background: {bar_bg};
    border-bottom: 1px solid rgba(255,255,255,.06);
  }}
  .dot {{ width: 11px; height: 11px; border-radius: 50%; }}
  .title {{
    flex: 1; text-align: center; margin-left: -57px;
    font: 500 12px -apple-system, "SF Pro Text", system-ui, sans-serif;
    color: rgba(255,255,255,.45); letter-spacing: .02em;
  }}
  pre {{
    margin: 0; padding: 16px 18px 18px;
    font-family: "SF Mono", Menlo, "DejaVu Sans Mono", monospace;
    font-size: 13px; line-height: 1.2;
    color: {fg}; white-space: pre; tab-size: 8;
  }}
</style></head><body>
  <div class="window">
    <div class="titlebar">
      <div class="dot" style="background:#ff5f57"></div>
      <div class="dot" style="background:#febc2e"></div>
      <div class="dot" style="background:#28c840"></div>
      <div class="title">openwith</div>
    </div>
    <pre>{body}</pre>
  </div>
</body></html>
"""


def to_html(frame):
    lines = frame.rstrip("\n").split("\n")
    out = []
    for line in lines:
        parts = []
        for style, text in sanitize_line(line):
            esc = html.escape(text)
            css = style.css()
            parts.append(f'<span style="{css}">{esc}</span>' if css else esc)
        out.append("".join(parts))
    return "\n".join(out)


CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def render(html_text, out_png):
    with tempfile.TemporaryDirectory() as tmp:
        page = os.path.join(tmp, "frame.html")
        with open(page, "w") as fh:
            fh.write(html_text)
        subprocess.run(
            [
                CHROME,
                "--headless",
                "--disable-gpu",
                "--hide-scrollbars",
                "--force-device-scale-factor=2",
                "--default-background-color=00000000",
                "--window-size=1200,900",
                f"--screenshot={out_png}",
                f"file://{page}",
            ],
            check=True,
            capture_output=True,
        )
    trim(out_png)


def trim(path):
    """Chrome screenshots the whole viewport; crop back to the rendered card."""
    from PIL import Image

    img = Image.open(path).convert("RGBA")
    bbox = img.split()[3].getbbox()
    if bbox:
        img.crop(bbox).save(path)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    src, dst = sys.argv[1], sys.argv[2]
    with open(src, "r") as fh:
        frame = fh.read()
    body = to_html(frame)
    page = PAGE.format(
        page_bg="#00000000",
        term_bg=BG_DEFAULT,
        bar_bg="#1b1f27",
        fg=FG_DEFAULT,
        body=body,
    )
    render(page, dst)
    print(f"    rendered {dst}")


if __name__ == "__main__":
    main()
