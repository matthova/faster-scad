---
"quito-release-root": minor
---

Web UI (M9 Phase 6): the playground is now usable on narrow screens. At ≥1024px the three resizable columns stand; below that the editor and viewer become a **Code ⎪ Model** segmented switch (Model — viewer + customizer — shown first, since read-and-tweak is the point at those widths), touch targets grow to 44px, the toolbar scrolls instead of overflowing, and phone widths shed secondary chrome. The core loop — edit, render, read the numbers, drag a parameter — works with no horizontal scrollbar at 1024, 820, and 480px. (Previously there were zero media queries and the app was unusable below desktop width.)
