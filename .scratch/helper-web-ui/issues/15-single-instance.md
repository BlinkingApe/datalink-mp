# 15: Single instance

**What to build:** A player who double-clicks the Helper a second time gets the page of the Helper that is already running, instead of a second Helper fighting over the same port. A tester who deliberately starts two Helpers on different IPC ports gets two Helpers, each with its own page.

Scope, from the spec's "Decisions this spec adds", "HTTP API" and "Further Notes" sections:

- **Keyed on the IPC port, with no files.** At startup the Helper binds the IPC port. If that fails with address-in-use, it asks each port in the UI range for `GET /api/instance`.
  - If a Helper answers with the same IPC port, the new process sends it `POST /api/show`, prints "datalink-mp is already running", and exits with status 0. The running Helper opens the browser itself, so the token never leaves it.
  - If no Helper answers, something else holds the port: the new process carries on and shows the `ipc_port_in_use` banner (ticket 09).
  - Two Helpers on different IPC ports are separate instances and both run; the second takes the next UI port through the port walk (ticket 06).
- **`GET /api/instance`**, no token. Returns the application name, the Release version and the IPC port.
- **`POST /api/show`**, no token. Makes this Helper open the browser at its own launch URL, through the configured browser opener. Limited to once every few seconds.
- **These two routes are the one deviation** from "every endpoint needs the token". They still get the `Host` check, and `POST /api/show` still rejects a foreign `Origin` and needs `Content-Type: application/json`. They reveal nothing secret and can at worst open a browser tab.
- The probing process must send a `Host` header the running Helper accepts.
- The library entry point reports "already running" as a distinct outcome so the binary can print the message and exit 0, and tests can assert on it.

Tests use the seam 1 harness with the recording browser opener.

**Blocked by:** 06 (Startup: browser opening, UI port walk, readable startup failure), 09 (Step 3: game link status)

**Status:** resolved

- [x] `GET /api/instance` without a token returns the application name, the Release version and the IPC port
- [x] `POST /api/show` without a token calls the browser opener with the launch URL
- [x] A second `POST /api/show` within the limit does not call the opener again
- [x] Both routes reject a wrong `Host`; `POST /api/show` rejects a foreign `Origin`
- [x] Neither route's response contains the token
- [x] A second start on the same IPC port returns "already running" and triggers the first Helper's opener
- [x] The built binary in that case prints "datalink-mp is already running" and exits with status 0
- [x] A second start on a different IPC port runs and takes the next UI port
- [x] A start while a non-Helper program holds the IPC port still gives a working UI with `ipc_port_in_use`
- [x] The first Helper is found when it is on a later port in the UI range, not only the default
- [x] The Helper writes no lock or state file
