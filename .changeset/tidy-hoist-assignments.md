---
"openrscad-release-root": patch
---

fix scope assignments to match OpenSCAD's last-write-wins semantics: a read of a variable reassigned later now sees its final value (`p = 1; q = p; p = 5;` gives `q == 5`), while variables introduced later are still not forward-referenced
