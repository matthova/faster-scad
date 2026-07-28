# Cherry-picks — sub-day items to bundle with any track

Each of these is under a day (most under an hour), doesn't deserve its own
milestone, and is worth doing immediately regardless of which track is chosen.

## 1. Link the live playground everywhere (~30 min)

The playground deployed successfully and is live at
<https://matthova.github.io/faster-scad/>, but nothing advertises it:

- README: put the URL in the first paragraph and in a badge; the current text
  says "The `web/` playground is live" without linking it, and the "Next
  steps" section still lists deploying as future work.
- GitHub repo settings: set the website field on the repo homepage
  (`gh repo edit --homepage https://matthova.github.io/faster-scad/`).

## 2. Fix `ci.yml` stale branch filter (~5 min)

The push trigger still references the merged branch `matthova/quito-v1`.
Should be `main` (PR trigger already covers feature branches).

## 3. README staleness pass (~1–2 h)

All verified stale as of `34ee83a`:

- **"Next steps" section is 100% pre-M2**: lists the Pages deploy (done, live),
  the Lezer grammar (done — `web/src/lang/openscad.grammar` +
  `web/src/lang/openscad.ts`, with highlighting/folding/indent), and
  "Begin M2" (M2 finished long ago). Replace with the current milestone
  pointer (this roadmap directory).
- **Repo layout table** still calls `desktop/` and `quito-engine` "planned" —
  desktop shipped.
- **"parse errors surfaced inline"** overstates: errors go to the status
  bar/console, not inline squiggles (see track B, item B2). Soften until B2
  lands.
- **BOSL2 "15/15"** vs 16 names in `xtask/src/main.rs` — pick the real number
  and make the harness print it (track A, item A6, makes this
  self-maintaining).

## 4. COMPAT.md divergence-register rewrite (~1–2 h)

The register no longer reflects reality and understates the good news while
hiding the real gaps:

- Row #6 ("geometry breadth: only cube/sphere/cylinder…") — **closed**; the
  full 2D/3D surface shipped in M3. Remove.
- "echo corpus 16/16" — now 24/24.
- Row #2 (assignment hoisting) — still open, keep (candidate fix in track A).
- Row #5 (`#`/`%` visual treatment) — still open, keep, pointing at track B
  item B3.
- **Add the real current divergences**, each with a repro, per the register's
  own policy: non-convex `minkowski()` = convex approximation; concave
  `offset()` self-intersections not clipped; `projection()` dropped inside 2D
  booleans; named-arg/axis-angle transforms unbound; `text(font=)` silently
  ignored (bundled Liberation Sans only); `color()` not rendered;
  `rands()` not bit-compatible (documented xorshift divergence).

Honest divergence docs are cheap trust: a switcher who finds a documented
limitation stays; one who finds an undocumented wrong answer leaves.

## 5. Quick CI additions that need no fallout work (~1 h)

`cargo fmt --check` and the web `tsc -b` PR job (track C items C4/C2) can land
with the branch-filter fix as a single "CI truthfulness" PR. Clippy is also
small but budget an hour for first-run fallout fixes.

## 6. Make `xtask bosl2` exit nonzero on 0/0 (~30 min)

Even before the full track-A/A6 treatment, the silent-0/0-success failure mode
(`let Ok(raw) … else { continue }` with an uninitialized submodule) is a
one-line guard: executed-count == 0 → exit 1 with a "did you
`git submodule update --init`?" hint. Prevents the metric from lying on any
machine, not just CI.
