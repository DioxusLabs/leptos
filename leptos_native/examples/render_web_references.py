"""Render test DOM snapshots in Chrome, without a wasm build or network."""

import base64
import json
import pathlib
import shutil
import subprocess
import tempfile

from websockets.sync.client import connect


def main():
    chrome = shutil.which("google-chrome") or shutil.which("chromium")
    if not chrome:
        raise SystemExit("google-chrome or chromium is required")
    output = pathlib.Path(__file__).resolve().parents[1] / "output"
    with tempfile.TemporaryDirectory() as profile:
        process = subprocess.Popen(
            [chrome, "--headless", "--no-sandbox", "--disable-gpu",
             "--hide-scrollbars", "--remote-debugging-port=0",
             "--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE localhost",
             f"--user-data-dir={profile}"],
            stderr=subprocess.PIPE, text=True,
        )
        try:
            for line in process.stderr:
                if "DevTools listening on " in line:
                    endpoint = line.split("DevTools listening on ", 1)[1].strip()
                    break
            else:
                raise RuntimeError("Chrome exited before publishing its CDP endpoint")
            with connect(endpoint) as socket:
                sequence = 0
                session = None

                def command(method, params=None):
                    nonlocal sequence
                    sequence += 1
                    message = {"id": sequence, "method": method, "params": params or {}}
                    if session:
                        message["sessionId"] = session
                    socket.send(json.dumps(message))
                    while True:
                        response = json.loads(socket.recv(timeout=30))
                        if response.get("id") == sequence:
                            if "error" in response:
                                raise RuntimeError(response["error"])
                            return response.get("result", {})

                target = command("Target.createTarget", {"url": "about:blank"})["targetId"]
                session = command("Target.attachToTarget", {"targetId": target, "flatten": True})["sessionId"]
                command("Page.enable")
                for app, width, height, states in [
                    ("benchmark", 1000, 700, ["before", "selected", "after"]),
                    ("todomvc", 720, 620, ["before", "completed", "after"]),
                ]:
                    command("Emulation.setDeviceMetricsOverride", {
                        "width": width, "height": height, "deviceScaleFactor": 1, "mobile": False,
                    })
                    for state in states:
                        source = output / f"{app}-{state}.html"
                        destination = output / f"{app}-{state}-web.png"
                        command("Page.navigate", {"url": source.as_uri()})
                        while json.loads(socket.recv(timeout=30)).get("method") != "Page.loadEventFired":
                            pass
                        command("Runtime.evaluate", {
                            "expression": "document.activeElement.blur()" if state != "before" else "void 0",
                        })
                        command("Runtime.evaluate", {
                            "expression": "new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))",
                            "awaitPromise": True,
                        })
                        png = command("Page.captureScreenshot", {"format": "png"})["data"]
                        destination.write_bytes(base64.b64decode(png))
                        print(destination, flush=True)
        finally:
            process.terminate()
            process.wait(timeout=10)


if __name__ == "__main__":
    main()
