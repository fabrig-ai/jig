"""Run a live command under a private adopter; settle its tree before returning.

The starter retains every direct child's unreaped identity and opens its pidfd.
Only adopted direct children are admitted, after checking the opened identity.
No cwd, PID file, parent discovery or process-group number grants authority.
A private stop file requests shutdown without signalling a saved shell $!.
"""
import argparse
import ctypes
import os
from pathlib import Path
import select
import signal
import subprocess
import time


def subreaper():
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(36, 1, 0, 0, 0) != 0:  # PR_SET_CHILD_SUBREAPER
        raise OSError(ctypes.get_errno(), "set private subreaper")
    value = ctypes.c_int()
    if libc.prctl(37, ctypes.byref(value), 0, 0, 0) != 0 or value.value != 1:
        raise RuntimeError("private adopter was not established")


def exited(fd):
    return bool(select.select([fd], [], [], 0)[0])


def retain(pid):
    """Open before checking membership; an exited/reused identity is refused."""
    if not 1 < pid <= 2**31 - 1:
        raise ValueError("refusing reserved PID")
    try:
        fd = os.pidfd_open(pid)
    except ProcessLookupError:
        return None
    try:
        stat = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
        if int(stat[1]) == os.getpid() and not exited(fd):
            return fd
    except (FileNotFoundError, ProcessLookupError):
        pass
    os.close(fd)
    return None


def send(fd, sig):
    try:
        signal.pidfd_send_signal(fd, sig)
    except ProcessLookupError:
        pass


def run(command, stop_file):
    subreaper()  # Established and checked before any child or sender starts.
    requested = False

    def stop(_sig, _frame):
        nonlocal requested
        requested = True

    for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(sig, stop)
    child = subprocess.Popen(command)
    # No poll/wait before acquiring this starter-owned, unreaped child.
    root = os.pidfd_open(child.pid)
    deadline = None
    status = None
    try:
        while True:
            if requested or stop_file.exists():
                if deadline is None:
                    send(root, signal.SIGTERM)
                    deadline = time.monotonic() + 10
                elif time.monotonic() >= deadline:
                    send(root, signal.SIGKILL)
            observed = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
            if observed is not None:
                break
            time.sleep(0.02)
        # The root stays unreaped until every adopted sender/descendant settles.
        # Killing one generation makes its children ours; repeat without ever
        # signalling a PID chosen from a different ownership domain.
        while True:
            children = [int(pid) for pid in Path("/proc/thread-self/children").read_text().split()]
            for pid in children:
                if pid == child.pid:
                    continue
                fd = retain(pid)
                if fd is not None:
                    try:
                        send(fd, signal.SIGKILL)
                    finally:
                        os.close(fd)
            for pid in children:
                if pid != child.pid:
                    os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG)
            if children == [child.pid]:
                break
            time.sleep(0.02)
        status = child.wait()
    finally:
        os.close(root)
    return status if status >= 0 else 128 - status


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stop-file", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command
    if command[:1] == ["--"]:
        command = command[1:]
    if not command:
        parser.error("a command is required")
    raise SystemExit(run(command, args.stop_file))


if __name__ == "__main__":
    main()
