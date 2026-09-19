#!/usr/bin/env bash
#
# Capture the README's TUI screenshots.
#
# Drives the real TUI inside a detached tmux session at a fixed size, grabs the
# rendered frames as ANSI text, and hands them to scripts/tui-shot.py, which
# swaps the local app roster for the demo one and renders PNGs.
#
# Nothing about the host machine reaches the frame: tmux renders off-screen, so
# there is no desktop, menu bar, shell prompt or window title in the capture.
#
# Usage:  ./scripts/capture-tui.sh [outdir]      (default: artifacts/)
#
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
outdir="${1:-$repo_root/artifacts}"
workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"; tmux kill-session -t openwith-shot 2>/dev/null || true' EXIT

cols=120
rows=34
# The extension scan takes a few seconds behind the loading screen; every frame
# is captured after the list is up.
boot_wait=14

cd "$repo_root"
echo "==> building the CLI"
cargo build --release -q -p openwith-cli
bin="$repo_root/target/release/openwith"

start_tui() {
  tmux kill-session -t openwith-shot 2>/dev/null || true
  tmux new-session -d -s openwith-shot -x "$cols" -y "$rows" "$bin"
  sleep "$boot_wait"
}

grab() {
  tmux capture-pane -t openwith-shot -e -p >"$workdir/$1.ansi"
  echo "    captured $1"
}

stop_tui() {
  tmux kill-session -t openwith-shot 2>/dev/null || true
}

# 1. Extensions list -- the hero shot: every extension on the machine, its
#    current default and the bundle ID behind it.
echo "==> frame: extensions"
start_tui
grab extensions

# 2. App picker -- the change flow. Opened straight onto the first row, a
#    video extension, so the candidate list is a set of media players.
#
#    The picker overlay clips whatever rows it covers, and clipped text cannot
#    be roster-swapped (the token is cut in half). It lands over the middle of
#    the list, where every default is an Apple stock app the roster keeps
#    as-is, so nothing that needs swapping is ever cut. Check that still holds
#    if this frame is re-aimed at a different row.
echo "==> frame: picker"
tmux send-keys -t openwith-shot Enter
sleep 2
grab picker
stop_tui

# 3. Apps browser -- master-detail. The selection is walked down to a media
#    player rather than left on the one-extension utility at the top of the
#    list, so the detail pane actually shows something. The walk stays inside
#    the visible window, so the list itself does not scroll.
echo "==> frame: apps"
start_tui
tmux send-keys -t openwith-shot Tab
sleep 2
for _ in $(seq 26); do
  tmux send-keys -t openwith-shot "j"
done
sleep 2
grab apps
stop_tui

echo "==> rendering"
mkdir -p "$outdir"
for frame in extensions picker apps; do
  python3 "$repo_root/scripts/tui-shot.py" \
    "$workdir/$frame.ansi" "$outdir/tui-$frame.png"
done

echo "==> done -> $outdir/tui-{extensions,picker,apps}.png"
