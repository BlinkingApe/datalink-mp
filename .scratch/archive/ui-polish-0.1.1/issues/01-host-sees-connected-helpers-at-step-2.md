# 01: The Host sees how many Helpers are connected at step 2

**What to build:** While the Helper is hosting, step 2 ("Share or paste a Ticket") shows how many Helpers are connected, under the Host's own Ticket and its Copy Ticket button. It uses step 4's wording ("1 Helper connected", "2 Helpers connected"), and it updates live as friends connect and drop. A hosting player can then see a friend's Ticket worked without scrolling to step 4. Step 4 is unchanged. A Joiner's step 2 is unchanged: it already says "Connected to your friend".

From the [post-0.1.0-polish map](../../../post-0.1.0-polish/map.md): [Show the host's connected-peer count earlier](../../../post-0.1.0-polish/issues/01-show-friends-connected-earlier.md).

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] While hosting with one or more peers, step 2 shows the count in step 4's wording
- [x] It isn't shown with no peers, or in any state but hosting
- [x] It follows the peer list as it changes, like step 4's line
- [x] The UI tests cover it
