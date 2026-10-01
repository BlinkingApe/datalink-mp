# 18: The Datalink look

**What to build:** The page has one look, whatever the browser's colour scheme, taken from the maintainer's SMACX-Datalink-inspired guide (`tools/0001-wtp-through-the-ages.html`, kept out of git): a dark phosphor-green terminal with scanlines over a faint grid, Chakra Petch headings, IBM Plex Sans text and IBM Plex Mono labels, and a card with a cut corner. It replaces the plain light/dark page, so the first release players see has it.

- The fonts are embedded in the Helper and put into the page as data URLs, so the page still loads nothing and works offline (ADR-0001: no external assets). The Content-Security-Policy gains `font-src data:` and nothing else.
- Chakra Petch has no Reserved Font Name, so it's cut down to Latin. IBM Plex reserves "Plex", so its files are IBM's own Latin1 and Pi web fonts, unmodified. `LICENSE-FONTS` covers both and ships in every archive as `datalink-mp-LICENSE-FONTS.txt`.
- No wording changes. The README images are re-rendered from the page the Helper serves.

**Blocked by:** None

**Status:** resolved

- [x] The served page embeds Chakra Petch, IBM Plex Sans and IBM Plex Mono as woff2 data URLs (`test_page_carries_its_fonts_inside_it`)
- [x] The policy allows fonts from data URLs only (`assert_csp_is_strict`)
- [x] The fonts load in a browser under the real policy (headless Firefox against a running Helper)
- [x] The README images show the new look
- [ ] Ticket 17's "narrow window" check passes on the new look
