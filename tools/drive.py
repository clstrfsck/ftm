#!/usr/bin/env python3
"""Drive the release binary on a pty and show what it drew (§8, §12).

Termino's terminal layer cannot be tested from `cargo test`: §17.1 is
explicitly "core, no terminal", and the shell is the half no unit test
reaches. This is the substitute -- a pty, a scripted burst of keystrokes,
and a toy ANSI interpreter that replays the capture into a character grid
so the screen can be *seen* without a human at a terminal.

    make -s release                       # this drives the release binary
    tools/drive.py                        # just look at the opening screen
    tools/drive.py c c                    # press hold twice
    tools/drive.py '\\x1b[B' '\\x1b[B' ' '  # down, down, hard drop
    tools/drive.py --legacy left left     # the §8.2 fallback path
    tools/drive.py enter resize:20x50     # shrink mid-game (§8.4, §12.1)
    tools/drive.py --arg=--seed=42        # the same game every time (§6.4)

Keys are Python string escapes, so '\\x1b[B' is Down and '\\r' is Enter;
a few names (up, down, left, right, space, enter, esc) are spelled out.
One frame is printed per keystroke, plus a final one after everything has
settled. A "key" of the form `resize:ROWSxCOLS` is not a key: it resizes
the pty and signals the child, which is the only way to reach §8.4's
resize handling and §12.1's too-small screen from here.

Three things to know before believing the output:

* Without `--arg=--seed=N` every run is a different game, so two runs are
  not comparable; press keys within one run and compare its frames, which
  is what the per-keystroke output is for. With a seed they *are*
  comparable, which is how a rendering change is told from a new game.
  `--arg` passes one argument to the binary and repeats. Write it glued on
  with `=`, both times: `--arg=--seed=42`, or `argparse` reads the value
  as an option of its own.
* The interpreter understands CUP, ED and text, and nothing else. It is
  enough for ratatui's diffed output and would mislead you about anything
  cleverer.
* Answering the §8.2 support queries makes the game take the enhanced
  keyboard path; `--legacy` declines them instead, so both §8.2 paths can
  be exercised on the same machine. Kitty key events are '\\x1b[1;1:1D'
  (press) and '\\x1b[1;1:3D' (release) if you need to send them by hand.
  A declined query is not answered but *timed out* inside crossterm, which
  costs about two seconds before the first frame -- so `--legacy` wants a
  longer `--settle` (2.5 s is enough) or its frames come back blank. That
  is the pty, not the game.

This does not replace playing it on a real terminal; it replaces guessing.
`stty -a` either side of a run is the A7 check (§17.3).
"""

import argparse
import fcntl
import os
import pty
import re
import select
import signal
import struct
import sys
import termios
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BINARY = os.path.join(REPO, "target", "release", "termino")

# Convenience spellings for the keys that are awkward as escapes.
NAMED = {
    "up": "\x1b[A",
    "down": "\x1b[B",
    "right": "\x1b[C",
    "left": "\x1b[D",
    "space": " ",
    "enter": "\r",
    "esc": "\x1b",
}

# The §8.2 capability queries and the replies a modern terminal gives.
QUERIES = [(b"\x1b[?u", b"\x1b[?1u"), (b"\x1b[c", b"\x1b[?62;22c")]


def resize(fd, pid, size):
    """Resize the pty and tell the child, which is what §8.4 reacts to."""
    rows, cols = size
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    os.kill(pid, signal.SIGWINCH)


def run(binary, keys, size, pause, settle, enhanced, argv=()):
    """Play `keys` through the binary, returning the capture after each one.

    A key of the form `("resize", (rows, cols))` is not a key at all: it
    resizes the pty and signals the child, which is the only way to reach
    §8.4 from here. Each frame is paired with the size in force when it was
    taken, because that is the geometry it has to be replayed at.
    """
    rows, cols = size
    pid, fd = pty.fork()
    if pid == 0:
        os.execv(binary, [os.path.basename(binary), *argv])
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))

    out = bytearray()

    def pump(seconds):
        until = time.time() + seconds
        while time.time() < until:
            ready, _, _ = select.select([fd], [], [], 0.05)
            if not ready:
                continue
            try:
                chunk = os.read(fd, 65536)
            except OSError:  # the child has gone
                return
            if not chunk:
                return
            out.extend(chunk)
            if enhanced:
                for query, reply in QUERIES:
                    if query in chunk:
                        os.write(fd, reply)

    frames = []
    pump(1.0)  # startup, and the capability handshake
    for item in keys:
        if isinstance(item, tuple):
            size = item[1]
            resize(fd, pid, size)
        else:
            os.write(fd, item.encode())
        pump(pause)
        frames.append((bytes(out), size))
    pump(settle)
    frames.append((bytes(out), size))

    os.write(fd, b"\x1b")  # quit, so the terminal is torn down properly (§8.3)
    pump(0.3)
    try:
        os.close(fd)
    except OSError:
        pass
    os.waitpid(pid, 0)
    return frames


def replay(data, size):
    """Replay a capture into a character grid. CUP, ED and text only."""
    rows, cols = size
    blank = lambda: [[" "] * cols for _ in range(rows)]
    grid = blank()
    cy = cx = 0
    i = 0
    while i < len(data):
        byte = data[i]
        if byte == 0x1B:
            csi = re.match(rb"\x1b\[([0-9;?]*)([A-Za-z])", data[i:])
            if not csi:
                # An OSC or DCS string: skip to its terminator and move on.
                string = re.match(rb"\x1b[\]P].*?(\x07|\x1b\\)", data[i:], re.S)
                i += string.end() if string else 2
                continue
            args, command = csi.group(1), csi.group(2)
            nums = [int(n) for n in args.split(b";") if n.isdigit()]
            if command == b"H":  # CUP, one-based
                cy = nums[0] - 1 if nums else 0
                cx = nums[1] - 1 if len(nums) > 1 else 0
            elif command == b"J":  # ED
                grid = blank()
            i += csi.end()
        elif byte in (0x0A, 0x0D):
            if byte == 0x0A:
                cy += 1
            else:
                cx = 0
            i += 1
        else:
            end = i
            while end < len(data) and data[end] not in (0x1B, 0x0A, 0x0D):
                end += 1
            for char in data[i:end].decode("utf-8", "replace"):
                if 0 <= cy < rows and 0 <= cx < cols:
                    grid[cy][cx] = char
                cx += 1
            i = end
    return "\n".join("".join(row).rstrip() for row in grid)


def key(spec):
    """A key from a name or a Python string escape, or a resize instruction."""
    if spec.lower().startswith("resize:"):
        rows, cols = (int(n) for n in spec.split(":", 1)[1].lower().split("x"))
        return ("resize", (rows, cols))
    if spec.lower() in NAMED:
        return NAMED[spec.lower()]
    return spec.encode("latin1", "backslashreplace").decode("unicode_escape")


def main():
    parser = argparse.ArgumentParser(
        description=__doc__.split("\n\n")[0],
        epilog="Pass --arg=--seed=42 for a game that is the same every run.",
    )
    parser.add_argument("keys", nargs="*", help="keys to send, one frame each")
    parser.add_argument("--size", default="30x100", help="ROWSxCOLS (default 30x100)")
    parser.add_argument("--pause", type=float, default=0.35, help="seconds per key")
    parser.add_argument("--settle", type=float, default=0.5, help="seconds at the end")
    parser.add_argument(
        "--legacy",
        action="store_true",
        help="decline the §8.2 capability queries, taking the fallback path",
    )
    parser.add_argument(
        "--arg",
        action="append",
        default=[],
        metavar="ARG",
        help="an argument to pass to the binary, repeatable; glue it on with "
        "= both times, as in --arg=--seed=42",
    )
    parser.add_argument("--bin", default=BINARY, help="the binary to drive")
    args = parser.parse_args()

    if not os.path.exists(args.bin):
        sys.exit(f"{args.bin} is not built: run `cargo build --release` first")
    rows, cols = (int(n) for n in args.size.lower().split("x"))
    size = (rows, cols)

    keys = [key(spec) for spec in args.keys]
    frames = run(args.bin, keys, size, args.pause, args.settle, not args.legacy, args.arg)
    for n, (frame, at) in enumerate(frames):
        label = f"after {args.keys[n]!r}" if n < len(keys) else "settled"
        print(f"--- {label} " + "-" * 40)
        print(replay(frame, at))


if __name__ == "__main__":
    main()
