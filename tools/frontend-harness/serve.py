"""Serve the real frontend against a stubbed backend, in an ordinary browser.

The window is a WebView with no console you can reach, so a JavaScript error
there is invisible: the UI just does nothing. This copies `app/src` next to a
stub for `window.__TAURI__` and serves it, so the same code can be driven in a
browser where errors, the DOM and every backend call are all visible.

    python tools/frontend-harness/serve.py        # then open the printed URL

Every invoke is logged to the console and recorded in `window.__calls`, so a
gesture that should reach the store but does not is visible as an absence.
"""

import http.server
import shutil
import socketserver
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
PORT = 8731


def build(dest: Path) -> None:
    for f in (ROOT / "app" / "src").iterdir():
        if f.suffix in (".js", ".css", ".html"):
            shutil.copy(f, dest / f.name)
    shutil.copy(Path(__file__).parent / "stub.js", dest / "stub.js")

    index = dest / "index.html"
    html = index.read_text(encoding="utf-8")
    tag = '<script type="module" src="main.js"></script>'
    if tag not in html:
        raise SystemExit("index.html no longer loads main.js the way this expects")
    index.write_text(
        html.replace(tag, '<script src="stub.js"></script>\n  ' + tag), encoding="utf-8"
    )


def main() -> None:
    dest = Path(tempfile.mkdtemp(prefix="ms-harness-"))
    build(dest)
    print(f"serving {dest}\n  http://127.0.0.1:{PORT}/index.html\nCtrl-C to stop")

    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **kw):
            super().__init__(*a, directory=str(dest), **kw)

        def log_message(self, *a):
            pass

    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as httpd:
        httpd.serve_forever()


if __name__ == "__main__":
    main()
