#!/usr/bin/env bash
# Drive a debug build of yazi headlessly inside Zellij, for testing TUI changes.
#
#   scripts/zellij-tui.sh start [dir]   # runs target/debug/yazi (cargo build first) with an isolated config
#   scripts/zellij-tui.sh key C-S-p     # send keys (see `send_key` below); several may be given
#   scripts/zellij-tui.sh type sort     # type literal text
#   scripts/zellij-tui.sh screen        # print the current screen
#   scripts/zellij-tui.sh stop          # tear everything down
#
# Zellij is the terminal emulator here, so yazi's capability queries get real answers.
# On Windows, `zellij attach --create-background` exits immediately (0.44.1), so an attached
# client is kept alive inside a pywinpty pseudo-terminal instead. On Unix, plain background mode works.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SESSION="${YAZI_TUI_SESSION:-yazi-tui}"
WORK="${YAZI_TUI_WORK:-${TMPDIR:-${TEMP:-/tmp}}/yazi-tui}"
DELAY="${YAZI_TUI_DELAY:-1}"

mkdir -p "$WORK/cfg"
# Empty Zellij config, so a broken or customized user config can't interfere
printf 'show_startup_tips false\nshow_release_notes false\npane_frames false\n' >"$WORK/zellij.kdl"
Z=(zellij --config "$WORK/zellij.kdl" -s "$SESSION")

is_windows() { [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* || "$(uname -s)" == CYGWIN* ]]; }

pause() { python -c "import time; time.sleep($1)"; }

session_alive() { zellij list-sessions --no-formatting 2>/dev/null | grep -v EXITED | grep -q "^$SESSION "; }

host_session() {
	if ! is_windows; then
		zellij --config "$WORK/zellij.kdl" attach --create-background "$SESSION"
		return
	fi

	local py="$WORK/venv/Scripts/python"
	if [[ ! -x "$py" ]]; then
		python -m venv "$WORK/venv"
		"$py" -m pip install -q pywinpty
	fi
	"$py" - "$WORK/zellij.kdl" "$SESSION" >"$WORK/host.log" 2>&1 <<-'EOF' &
		import sys, winpty
		p = winpty.PtyProcess.spawn(["zellij", "--config", sys.argv[1], "attach", "--create", sys.argv[2]], dimensions=(30, 110))
		while p.isalive():
		    try: p.read(65536)
		    except EOFError: break
	EOF
	echo $! >"$WORK/host.pid"
}

send_key() {
	case "$1" in
		C-S-p) "${Z[@]}" action write 27 91 49 49 50 59 54 117 ;; # ESC [112;6u (kitty protocol)
		C-p) "${Z[@]}" action write 16 ;;
		enter) "${Z[@]}" action write 13 ;;
		tab) "${Z[@]}" action write 9 ;;
		S-tab) "${Z[@]}" action write 27 91 90 ;; # ESC [Z
		esc) "${Z[@]}" action write 27 91 50 55 117 ;; # ESC [27u, unambiguous unlike a bare ESC
		up) "${Z[@]}" action write 27 91 65 ;;
		down) "${Z[@]}" action write 27 91 66 ;;
		backspace) "${Z[@]}" action write 127 ;;
		*) "${Z[@]}" action write-chars "$1" ;; # Any other single key, e.g. `q` or `.`
	esac
}

case "${1:-}" in
	start)
		dir="${2:-$WORK/play}"
		mkdir -p "$WORK/play/sub" && touch "$WORK/play/"{a.md,b.txt,c.rs,.hidden}
		session_alive || host_session
		for _ in $(seq 1 40); do session_alive && break; pause 0.25; done
		session_alive || { echo "Zellij session failed to start" >&2; exit 1; }
		pause 1
		"${Z[@]}" action write-chars "YAZI_CONFIG_HOME='$WORK/cfg' YAZI_LOG=debug '$ROOT/target/debug/yazi' '$dir'"
		"${Z[@]}" action write 13
		pause 5
		"${Z[@]}" action dump-screen
		;;
	key)
		shift
		for k in "$@"; do send_key "$k"; pause "$DELAY"; done
		"${Z[@]}" action dump-screen
		;;
	type)
		"${Z[@]}" action write-chars "$2"
		pause "$DELAY"
		"${Z[@]}" action dump-screen
		;;
	screen)
		"${Z[@]}" action dump-screen
		;;
	stop)
		zellij kill-session "$SESSION" 2>/dev/null || true
		zellij delete-session "$SESSION" 2>/dev/null || true
		[[ -f "$WORK/host.pid" ]] && kill "$(cat "$WORK/host.pid")" 2>/dev/null || true
		rm -f "$WORK/host.pid"
		;;
	*)
		sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
		exit 1
		;;
esac
