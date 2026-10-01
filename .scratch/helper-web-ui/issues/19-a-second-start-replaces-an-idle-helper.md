# 19: A second start replaces an idle Helper

**What to build:** Found during the 0.1.0 gate. A player who extracted a new release over a running Helper, then double-clicked it, got the old Helper's page: the old build kept running, and its state kept causing trouble (release-pipeline ticket 06). Now a second start on the same IPC port asks the running Helper to make way:

- **Nothing uses it** (no game connected, no friend connected, no join under way): it quits as Quit does, and the new Helper starts once the IPC port is free.
- **The game or a friend is connected:** it keeps running, so nothing is interrupted. Its page opens with an amber "Already running" notice for a minute, saying why and how to restart it, and the new process says so and exits with status 0.

The page footer, status, `GET /api/instance` and the console name the build (the short commit hash), so two builds of one Release version, such as two RCs, can be told apart.

**Blocked by:** None

**Status:** resolved

- [x] An idle first Helper makes way, and the second serves the game (`test_second_start_on_the_same_ipc_port_replaces_an_idle_first_helper`, `test_binary_started_again_replaces_an_idle_running_one`)
- [x] A first Helper the game is connected to stays, opens its page and shows `restart_refused`; so does one a friend is connected to
- [x] `POST /api/replace` follows the same Host, Origin and content-type rules as `/api/show`
- [x] The build is named in `/api/instance`, status, the footer and the console
- [ ] Checked by hand on the next RC: extract a new build over a running Helper, double-click it, and the footer shows the new build
