#!/usr/bin/env python3
"""Exercise one complete Typeul session through a real Unix PTY."""

import errno
import json
import os
from pathlib import Path
import select
import signal
import struct
import subprocess
import sys
import tempfile
import time


MAX_OUTPUT = 1024 * 1024
LIFECYCLE_SECONDS = 10.0
SESSION_KEYS = {
    "accuracy",
    "attempted_units",
    "backspaces",
    "content_id",
    "correct_units",
    "cpm",
    "difficulty",
    "duration_ms",
    "errors",
    "id",
    "intended_keys",
    "kpm",
    "language",
    "local_date",
    "mode",
    "schema_version",
    "started_at_unix_ms",
    "wpm",
}


class SmokeFailure(Exception):
    pass


class PtyUnavailable(Exception):
    pass


def require(condition, message):
    if not condition:
        raise SmokeFailure(message)


def assert_absent(value, secrets):
    if isinstance(value, dict):
        strings = list(value.keys())
        values = value.values()
    elif isinstance(value, list):
        strings = []
        values = value
    else:
        strings = [value] if isinstance(value, str) else []
        values = []
    for string in strings:
        folded = string.casefold()
        for secret in secrets:
            require(secret.casefold() not in folded, "session JSON leaked private test data")
    for nested in values:
        assert_absent(nested, secrets)


def self_check_privacy_guard():
    try:
        assert_absent({"aggregate": ["prefix-hello-suffix"]}, ["hello"])
    except SmokeFailure:
        return
    raise SmokeFailure("privacy assertion self-check did not catch a mutated session")


def remaining(deadline, stage):
    seconds = deadline - time.monotonic()
    if seconds <= 0:
        raise SmokeFailure("timed out " + stage)
    return seconds


def read_once(master, output, deadline, stage):
    readable, _, _ = select.select(
        [master], [], [], min(0.05, remaining(deadline, stage))
    )
    if not readable:
        return 0
    try:
        chunk = os.read(master, min(65536, MAX_OUTPUT - len(output) + 1))
    except OSError as error:
        if error.errno == errno.EIO:
            return -1
        raise
    if not chunk:
        return -1
    output.extend(chunk)
    require(len(output) <= MAX_OUTPUT, "terminal output exceeded 1 MiB")
    return 1


def wait_for(master, child, output, needles, deadline, stage):
    while not all(needle in output for needle in needles):
        state = read_once(master, output, deadline, stage)
        if state < 0:
            raise SmokeFailure("terminal closed " + stage)
        if state == 0 and child.poll() is not None:
            raise SmokeFailure("process exited " + stage)


def write_all(master, data, deadline, stage):
    written = 0
    while written < len(data):
        _, writable, _ = select.select(
            [], [master], [], min(0.05, remaining(deadline, stage))
        )
        if writable:
            count = os.write(master, data[written:])
            require(count > 0, "terminal write made no progress")
            written += count


def wait_for_exit(master, child, output, deadline):
    closed = False
    while True:
        if not closed:
            closed = read_once(master, output, deadline, "waiting for process exit") < 0
        status = child.poll()
        if status is not None and (closed or not select.select([master], [], [], 0)[0]):
            return status


def kill_and_reap(child):
    if child is None:
        return
    # Every caller starts an owned session whose process-group ID is child.pid.
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except OSError:
        child.kill()
    try:
        child.wait(timeout=1)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait()


def self_check_process_group_cleanup():
    leader = subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import os,time; os.fork() == 0 and time.sleep(30)",
        ],
        start_new_session=True,
    )
    group = leader.pid
    try:
        leader.wait(timeout=1)
        os.killpg(group, 0)
        kill_and_reap(leader)
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            try:
                os.killpg(group, 0)
            except ProcessLookupError:
                return
            except PermissionError:
                pass
            time.sleep(0.01)
        raise SmokeFailure("process-group cleanup left a surviving descendant")
    finally:
        try:
            os.killpg(group, signal.SIGKILL)
        except (PermissionError, ProcessLookupError):
            pass
        try:
            leader.wait(timeout=1)
        except subprocess.TimeoutExpired:
            leader.kill()
            leader.wait()


def close_fd(fd):
    if fd is not None:
        try:
            os.close(fd)
        except OSError:
            pass


def validate_session(home, target, material_root):
    sessions = sorted((home / "sessions").glob("*.json"))
    require(len(sessions) == 1, "expected exactly one session JSON")
    try:
        record = json.loads(sessions[0].read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise SmokeFailure("invalid session JSON: " + str(error)) from error
    require(isinstance(record, dict), "session JSON must be an object")
    require(set(record) == SESSION_KEYS, "unexpected session schema: " + repr(sorted(record)))
    require(record["schema_version"] == 1, "unexpected session schema version")
    require(record["id"] == sessions[0].stem, "session ID does not match its filename")
    require(record["correct_units"] == 5, "expected five correct units")
    require(record["attempted_units"] == 5, "expected five attempted units")
    require(record["errors"] == 0, "expected zero errors")
    assert_absent(
        record,
        ["hello", str(target), str(material_root), str(home)],
    )
    return sessions[0], sorted(record)


def open_pty(pty, fcntl, termios):
    try:
        master, slave = pty.openpty()
    except OSError as error:
        if error.errno in {
            errno.ENODEV,
            errno.ENOENT,
            errno.ENOSYS,
            getattr(errno, "EOPNOTSUPP", errno.ENOSYS),
        }:
            raise PtyUnavailable(str(error)) from error
        raise
    try:
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    except Exception:
        close_fd(master)
        close_fd(slave)
        raise
    return master, slave


def run_smoke(binary, pty, fcntl, termios):
    material_path = None
    home_path = None
    result = None
    with tempfile.TemporaryDirectory(prefix="typeul-pty-material-") as material_tmp:
        with tempfile.TemporaryDirectory(prefix="typeul-pty-home-") as home_tmp:
            material_root = Path(material_tmp).resolve()
            home = Path(home_tmp).resolve()
            target = (material_root / "hello").resolve()
            target.write_text("hello", encoding="utf-8")
            material_path = material_root
            home_path = home

            master = None
            slave = None
            child = None
            output = bytearray()
            started = time.monotonic()
            deadline = started + LIFECYCLE_SECONDS
            try:
                master, slave = open_pty(pty, fcntl, termios)
                environment = os.environ.copy()
                environment.pop("LC_ALL", None)
                environment.update(
                    {
                        "LANG": "en_US.UTF-8",
                        "TYPEUL_HOME": str(home),
                        "TYPEUL_NO_UPDATE_CHECK": "1",
                    }
                )
                child = subprocess.Popen(
                    [str(binary), "practice", str(target)],
                    stdin=slave,
                    stdout=slave,
                    stderr=slave,
                    env=environment,
                    close_fds=True,
                    start_new_session=True,
                )
                close_fd(slave)
                slave = None

                wait_for(
                    master,
                    child,
                    output,
                    [b"Progress", b"hello"],
                    deadline,
                    "waiting for the practice screen",
                )
                write_all(master, b"hello", deadline, "typing the target")
                wait_for(
                    master,
                    child,
                    output,
                    [b"Result"],
                    deadline,
                    "waiting for the result screen",
                )
                write_all(master, b"\x1b", deadline, "leaving the result screen")
                wait_for(
                    master,
                    child,
                    output,
                    [b"Quick practice"],
                    deadline,
                    "waiting for the home screen",
                )
                write_all(master, b"\x1b", deadline, "leaving Typeul")
                status = wait_for_exit(master, child, output, deadline)
                require(status == 0, "Typeul exited with status " + str(status))

                require(b"\x1b[?1049h" in output, "alternate screen was not entered")
                cleanup = b"\x1b[?2004l\x1b[?1049l\x1b[?25h"
                require(cleanup in output, "terminal cleanup sequence was incomplete or unordered")
                session, schema = validate_session(home, target, material_root)
                result = (session.name, schema, time.monotonic() - started)
            except BaseException:
                kill_and_reap(child)
                if output:
                    sys.stderr.write(
                        "terminal tail:\n"
                        + bytes(output[-4096:]).decode("utf-8", errors="replace")
                        + "\n"
                    )
                raise
            finally:
                kill_and_reap(child)
                close_fd(slave)
                close_fd(master)

    require(material_path is not None and not material_path.exists(), "material temp leaked")
    require(home_path is not None and not home_path.exists(), "home temp leaked")
    return result


def main():
    if os.name == "nt":
        print("pty smoke skipped: Unix PTYs are unavailable on Windows")
        return 0
    try:
        import fcntl
        import pty
        import termios
    except ImportError as error:
        print("pty smoke skipped: OS PTY support is unavailable: " + str(error))
        return 0
    if not hasattr(termios, "TIOCSWINSZ"):
        print("pty smoke skipped: OS PTY window sizing is unavailable")
        return 0
    if len(sys.argv) != 2:
        print("usage: pty-smoke.py BINARY", file=sys.stderr)
        return 2

    try:
        self_check_privacy_guard()
        self_check_process_group_cleanup()
        binary = Path(sys.argv[1]).resolve(strict=True)
        require(binary.is_file(), "binary path is not a file")
        session, schema, elapsed = run_smoke(binary, pty, fcntl, termios)
    except PtyUnavailable as error:
        print("pty smoke skipped: OS PTY support is unavailable: " + str(error))
        return 0
    except (OSError, SmokeFailure, subprocess.SubprocessError) as error:
        print("pty smoke failed: " + str(error), file=sys.stderr)
        return 1

    print(
        "pty smoke ok: "
        + session
        + ", correct_units=5, errors=0, cleanup restored, schema="
        + ",".join(schema)
        + ", elapsed={:.2f}s".format(elapsed)
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
