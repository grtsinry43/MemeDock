#!/usr/bin/env python3
"""Real Android/Linux acceptance; requires a disposable memedock_acceptance_* AVD.

Run: python3 tools/testing/android-linux-sync.py --serial emulator-5580
Only the named disposable AVD is cleared. Libraries, credentials and logs use a
private temporary directory outside the repository. No third-party Python packages.
"""
import argparse
import base64
import http.cookiejar
import json
import os
from pathlib import Path
import socket
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

REPO = Path(__file__).resolve().parents[2]
APP = "com.grtsinry43.memedock"
RUNNER = f"{APP}.test/androidx.test.runner.AndroidJUnitRunner"
GTK_TEST = "settings::tests::settings_and_language_bindings_work_in_a_real_gtk_context"
ORIGIN = "http://127.0.0.1:18787"


def run(args, *, cwd=REPO, env=None, timeout=300, input=None):
    # Session D-Bus services may inherit stdout and outlive the command. A file
    # avoids waiting for those descendants to close a captured pipe after exit.
    with tempfile.TemporaryFile(mode="w+") as log:
        result = subprocess.run([str(arg) for arg in args], cwd=cwd, env=env,
                                input=input, text=True, stdout=log, stderr=subprocess.STDOUT,
                                timeout=timeout)
        log.seek(0)
        output = log.read()
    if result.returncode:
        raise RuntimeError(f"{args[0]} failed ({result.returncode}):\n{output}")
    return output


def binary(package, target):
    output = run(["cargo", "test", "--locked", "-p", package, "--no-run", "--message-format=json"])
    for line in output.splitlines():
        if not line.startswith("{"):
            continue
        value = json.loads(line)
        if value.get("target", {}).get("name") == target and value.get("executable"):
            return value["executable"]
    raise RuntimeError(f"missing executable: {target}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", required=True, help="disposable memedock_acceptance_* emulator")
    parser.add_argument("--skip-build", action="store_true", help="use already built Android APKs")
    parser.add_argument("--regression", action="store_true", help="also run all Android app device tests")
    args = parser.parse_args()
    os.umask(0o077)
    root = Path(tempfile.mkdtemp(prefix="memedock-cross-"))
    print(f"Private test artifacts: {root}", flush=True)
    adb = ["adb", "-s", args.serial]
    avd = run(adb + ["emu", "avd", "name"]).splitlines()[0].strip()
    if not args.serial.startswith("emulator-") or not avd.startswith("memedock_acceptance_"):
        raise RuntimeError("refusing to modify an emulator that is not a disposable acceptance AVD")
    deadline = time.monotonic() + 180
    while run(adb + ["shell", "getprop", "sys.boot_completed"]).strip() != "1":
        if time.monotonic() > deadline:
            raise RuntimeError("disposable emulator did not finish booting")
        time.sleep(1)
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(("127.0.0.1", 18787))  # Refuse to connect tests to an existing server.
    if not args.skip_build:
        tasks = [":app:assembleDebug", ":app:assembleDebugAndroidTest"]
        if args.regression:
            tasks.append(":shareTestApp:assembleDebug")
        build = run(["./gradlew", *tasks, "-Pmemedock.abis=x86_64"], cwd=REPO / "apps/android", timeout=900)
        (root / "android-build.log").write_text(build)
    for apk in ["apk/debug/app-debug.apk", "apk/androidTest/debug/app-debug-androidTest.apk"]:
        run(adb + ["install", "-r", "-t", REPO / "apps/android/app/build/outputs" / apk])
    if args.regression:
        run(adb + ["install", "-r", "-t", REPO / "apps/android/shareTestApp/build/outputs/apk/debug/shareTestApp-debug.apk"])
    run(adb + ["shell", "pm", "clear", APP])
    run(adb + ["reverse", "tcp:18787", "tcp:18787"])
    linux = binary("memedock-linux", "memedock")
    server = binary("memedock-server", "auth_devices")
    cookie = http.cookiejar.CookieJar()
    client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cookie))
    config = {"root": str(root), "origin": ORIGIN}

    def request(path, body=None, method=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(ORIGIN + "/api/v1" + path, data=data, method=method,
                                     headers={"Content-Type": "application/json", "Origin": ORIGIN})
        with client.open(req, timeout=10) as response:
            return json.load(response)["data"]

    def provision(platform):
        data = request("/admin/devices", {"name": f"Cross {platform}"})
        config[platform + "Token"] = data["token"]
        config[platform + "Device"] = data["device"]["id"]

    count = 0

    def phase(platform, name):
        nonlocal count, config
        count += 1
        config["phase"] = name
        print(f"[{count}] {platform}: {name}", flush=True)
        run(adb + ["shell", "am", "force-stop", APP])
        if platform == "linux":
            file = root / "config.json"
            file.write_text(json.dumps(config))
            environment = dict(os.environ, MEMEDOCK_CROSS_CONFIG=str(file), GDK_BACKEND="x11",
                               GSK_RENDERER="cairo", GTK_A11Y="none",
                               XDG_DATA_HOME=str(root / "xdg-data"), XDG_CACHE_HOME=str(root / "xdg-cache"),
                               XDG_CONFIG_HOME=str(root / "xdg-config"))
            try:
                output = run(["dbus-run-session", "--", "xvfb-run", "-a", linux,
                              GTK_TEST, "--ignored", "--exact", "--nocapture"], env=environment, timeout=180)
            except RuntimeError as error:
                (root / f"{count:02}-{platform}-{name}.log").write_text(str(error))
                raise
            config = json.loads(file.read_text())
            if name == "seed":
                config["png"] = base64.b64encode((root / "seed.png").read_bytes()).decode()
        else:
            run(adb + ["shell", "run-as", APP, "mkdir", "-p", "files"])
            run(adb + ["shell", "run-as", APP, "tee", "files/cross-platform.json"], input=json.dumps(config))
            command = adb + ["shell", "am", "instrument", "-w", "-r", "-e", "class",
                             f"{APP}.feature.CrossPlatformSyncDeviceTest", RUNNER]
            if name == "interrupt":
                run(adb + ["shell", "run-as", APP, "rm", "-f", "files/cross-interrupt-ready"])
                with (root / f"{count:02}-android-interrupt.log").open("w") as log:
                    instrument = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
                    try:
                        deadline = time.monotonic() + 90
                        while True:
                            marker = run(adb + ["shell", "run-as", APP, "ls", "files"])
                            if "cross-interrupt-ready" in marker.splitlines():
                                break
                            if instrument.poll() is not None or time.monotonic() > deadline:
                                raise RuntimeError("Android did not reach in-flight sync checkpoint")
                            time.sleep(0.1)
                    finally:
                        run(adb + ["shell", "am", "force-stop", APP])
                        instrument.wait(timeout=15)
                return
            output = run(command, timeout=240)
            (root / f"{count:02}-{platform}-{name}.log").write_text(output)
            if "OK (1 test)" not in output or "FAILURES" in output or "INSTRUMENTATION_FAILED" in output:
                raise RuntimeError(output)
            config = json.loads(run(adb + ["shell", "run-as", APP, "cat", "files/cross-result.json"]))
        (root / f"{count:02}-{platform}-{name}.log").write_text(output)

    process = None
    try:
        with (root / "server.log").open("w") as log:
            process = subprocess.Popen([server, "browser_fixture", "--ignored", "--exact", "--nocapture"],
                                       cwd=REPO, stdout=log, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + 30
        while True:
            if process.poll() is not None:
                raise RuntimeError("server exited: " + (root / "server.log").read_text())
            try:
                request("/admin/me")
            except urllib.error.HTTPError as error:
                if error.code == 401:
                    break
                raise
            except urllib.error.URLError:
                if time.monotonic() > deadline:
                    raise RuntimeError("server did not start") from None
                time.sleep(0.1)
        request("/admin/login", {"username": "admin", "password": "memedock-test-password"})
        provision("linux")
        provision("android")
        for platform, name in [
            ("linux", "seed"), ("android", "edit"), ("linux", "edited"),
            ("android", "offline"), ("linux", "offline"), ("android", "resume"),
            ("linux", "merge"), ("android", "merged"),
            ("linux", "delete"), ("android", "restore"), ("linux", "restored"),
            ("linux", "purge"), ("android", "reimport"), ("linux", "fresh"),
        ]:
            phase(platform, name)
        run(adb + ["reverse", "--remove", "tcp:18787"])
        try:
            phase("android", "transport")
        finally:
            run(adb + ["reverse", "tcp:18787", "tcp:18787"])
        config["expectedNote"] = "network-recovered"
        phase("android", "network-resume")
        phase("linux", "network-verify")
        process.send_signal(signal.SIGSTOP)
        try:
            phase("android", "interrupt")
            phase("linux", "interrupt")
        finally:
            process.send_signal(signal.SIGCONT)
        config["expectedNote"] = "interrupted-android"
        phase("android", "network-resume")
        phase("linux", "network-verify")
        for platform in ("android", "linux"):
            request("/admin/devices/" + config[platform + "Device"], method="DELETE")
            phase(platform, "revoked")
            provision(platform)
            phase(platform, "reauthorize")
        phase("linux", "export-backup")
        config["linuxArchive"] = base64.b64encode((root / "linux-backup.memedock").read_bytes()).decode()
        phase("android", "backup")
        # Transfer the real portable archive through adb without interpreting its contents.
        with (root / "android-backup.memedock").open("wb") as archive:
            subprocess.run(adb + ["exec-out", "run-as", APP, "cat", "files/share/cross-platform-backup.memedock"],
                           stdout=archive, check=True, timeout=30)
        phase("linux", "backup")
        print(f"PASS: {count} real cross-platform phases", flush=True)
        if args.regression:
            run(adb + ["shell", "am", "force-stop", APP])
            # Preserve the tested library; clear only the external phase inputs.
            # pm clear also removes recent tasks asynchronously and can kill a
            # newly launched instrumentation process after reporting success.
            run(adb + ["shell", "run-as", APP, "rm", "-f", "files/cross-platform.json", "files/cross-result.json"])
            provision("android")
            output = run(adb + ["shell", "am", "instrument", "-w", "-r", "-e", "syncOrigin", ORIGIN,
                               "-e", "syncToken", config["androidToken"], RUNNER], timeout=900)
            (root / "android-regression.log").write_text(output)
            if "OK (" not in output or "FAILURES" in output or "INSTRUMENTATION_FAILED" in output:
                raise RuntimeError(output)
            summary = next(line for line in output.splitlines() if line.startswith("OK ("))
            skipped = output.count("INSTRUMENTATION_STATUS_CODE: -4")
            print(f"Android regression: {summary}; {skipped} assumption skip(s)", flush=True)
    finally:
        if process is not None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        # Configurations contain disposable tokens; do not retain them with diagnostics.
        (root / "config.json").unlink(missing_ok=True)
        run(adb + ["shell", "am", "force-stop", APP])
        run(adb + ["shell", "run-as", APP, "rm", "-f", "files/cross-platform.json", "files/cross-result.json"])
        run(adb + ["reverse", "--remove", "tcp:18787"])


if __name__ == "__main__":
    main()
