# Driving the frontend outside the app

The app window is a WebView with no reachable console. When a gesture silently
does nothing, that is usually a JavaScript exception you cannot see — and a
screenshot of the native window cannot tell you the difference between "the
handler never ran" and "the handler threw".

```sh
python tools/frontend-harness/serve.py
```

Then open the printed URL. It serves the real `app/src` with `stub.js` standing
in for the Tauri backend: `window.__TAURI__.core.invoke` answers with fixed data
and records every call.

Useful in the page console:

```js
window.__calls        // every invoke, in order, with its arguments
window.__state        // the app's own state object
```

The stub serves an invented term — a few courses, a gym habit, a to-do list
with a running timer — which is what `docs/screenshots/` is made from. Add
`?empty` to the URL for a blank week instead, which is what you want when
testing a gesture that needs a free hour to click into.

None of it comes from a real store. It is a fixture, not a backup.

This found the bug where clicking an empty hour opened a box that could not
post: `Element.remove()` on the composer blurred the input inside it, which
re-entered `closeSlot` through `onblur`, and the second removal threw.
