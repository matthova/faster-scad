# Changelog

## 0.4.1

### Patch Changes

- [#58](https://github.com/matthova/faster-scad/pull/58) [`18b4553`](https://github.com/matthova/faster-scad/commit/18b4553aa964277b4a45548898abaa912b7ed478) Thanks [@matthova](https://github.com/matthova)! - fix crash-recovery so a too-heavy model can't freeze the app on every launch: the render watchdog no longer clears its own recovery sentinel, and safe mode now survives repeated relaunches until a render actually finishes (previously a render heavier than the 20s watchdog re-triggered the freeze on startup, since the watchdog's own timeout wiped the "skip auto-render" flag)

## 0.4.0

### Minor Changes

- [#56](https://github.com/matthova/faster-scad/pull/56) [`d6abf41`](https://github.com/matthova/faster-scad/commit/d6abf4158500549b9c6846c1eb82db180b275613) Thanks [@matthova](https://github.com/matthova)! - recover from heavy-geometry render freezes: a render-in-progress indicator with a Stop button, a watchdog that auto-stops runaway renders, and a startup recovery banner that skips auto-rendering a project whose last render never finished (so a too-heavy script no longer freezes the app on every reload)

## 0.3.0

### Minor Changes

- [#54](https://github.com/matthova/faster-scad/pull/54) [`2776978`](https://github.com/matthova/faster-scad/commit/2776978288df61fabe157a96026665d0c65a62ff) Thanks [@matthova](https://github.com/matthova)! - viewer: add a zoom-adaptive reference grid with numeric X/Y/Z axis labels and ruler ticks (spacing steps by powers of ten as you zoom), plus a draggable navigation cube in the top-right — drag it to orbit, or click a face, edge, or corner to fly to that face-on, 45°, or isometric view

## [0.2.0](https://github.com/matthova/faster-scad/compare/v0.1.1...v0.2.0) (2026-08-01)

### Features

- **geom,web:** render non-manifold models via weld + graceful CSG degradation ([#47](https://github.com/matthova/faster-scad/issues/47)) ([011fcfb](https://github.com/matthova/faster-scad/commit/011fcfbc2bcb7139532814a6cf0f926ebb75ab93))
- **npm:** publish the wasm engine as `quito-engine` ([#48](https://github.com/matthova/faster-scad/issues/48)) ([4b2eda7](https://github.com/matthova/faster-scad/commit/4b2eda7f38d7348a9cc1b693f0ae4ac2d4ba68f3))
