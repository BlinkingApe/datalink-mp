# 07: HTTP security hardening and token handling in the page

**What to build:** Only the player's own browser tab can control the Helper. Another program, another web page, or a malicious site using DNS tricks cannot read status or press buttons, and the secret does not linger in the address bar or browser history.

Scope, from the spec's "Security", "HTTP API" and "The page" sections (ADR-0001 acceptance criterion 5). Ticket 05 already binds `127.0.0.1` and checks the token on status. This ticket adds the remaining checks as rules that cover every route, including routes added by later tickets:

- **`Host` check.** Every request's `Host` header must be `127.0.0.1:<ui-port>` or `localhost:<ui-port>`, where the port is the one actually bound. Otherwise 403. This applies to `GET /` and to unauthenticated routes as well.
- **POST rules.** State-changing routes are POST only, need `Content-Type: application/json`, and reject any `Origin` that is present and is not the Helper's own.
- **Token.** Compared in constant time.
- **No CORS headers** are ever sent.
- **Response headers.** `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, and a Content-Security-Policy that allows only the page's own inline script and style, connections to itself, and no framing.
- **Token handling in the page.** The page takes the token from `?t=`, keeps it in `sessionStorage` so a reload works, and removes it from the address bar with `history.replaceState`.
- **Stale tab.** If a poll gets 403, the page shows "This tab is from an earlier run of datalink-mp. Use the new tab, or double-click datalink-mp again."

No POST route may exist yet when this ticket is picked up. Build the POST rules so they apply to any POST route, and test them against a route that exists or a test-only one. Each later ticket that adds a POST route re-asserts the rules for its own route.

Tests use the seam 1 harness from ticket 05 and assert only on HTTP responses.

**Blocked by:** 05 (UI mode tracer bullet)

**Status:** resolved

- [ ] A request whose `Host` is another name gets 403, on `GET /` and on `GET /api/status`
- [ ] A request whose `Host` has the right name and another port gets 403
- [ ] `Host: 127.0.0.1:<bound port>` and `Host: localhost:<bound port>` are both accepted
- [ ] A POST with a foreign `Origin` gets 403; a POST with no `Origin`, or with the Helper's own, passes this check
- [ ] A POST without `Content-Type: application/json` is rejected
- [ ] A GET on a POST route is rejected
- [ ] No response carries any `Access-Control-*` header, including the response to an `OPTIONS` preflight
- [ ] Every response carries `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer` and the Content-Security-Policy described above
- [ ] The page still loads and polls under that Content-Security-Policy
- [ ] The token comparison is constant-time
- [ ] After the page loads, the address bar no longer shows `?t=`, and reloading the tab keeps working
- [ ] A poll answered with 403 replaces the page with the earlier-run message
