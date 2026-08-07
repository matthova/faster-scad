// Last-write-wins scope semantics: within a scope only a variable's final
// assignment is evaluated, at the point it was first introduced. There are no
// forward references — a read of a name introduced later falls through to an
// outer binding or undef.

// A read of a variable reassigned later sees its final value, not the
// intermediate one.
p = 1; q = p; p = 5; echo("lastwin", q, p);

// The last of several rewrites wins.
r = 1; r = 2; s = r; r = 3; echo("multi", s, r);

// No forward references: reads of later-introduced names are undef at top level.
fy = fx; fx = 5; echo("forward", fy, fx);
ca = cb + 1; cb = cc * 2; cc = 10; echo("chain", ca, cb, cc);

// The surviving assignment is evaluated at first-introduction, so it cannot see
// a variable introduced afterwards.
ja = 1; jc = ja; jb = 2; ja = jb; echo("survivor", ja, jb, jc);

// Self-reference in the surviving assignment is undef, not infinite.
self = self + 1; echo("selfref", self);

// An overwritten assignment's RHS is discarded, side effects included.
dead = echo("dead-ran") 1; dead = 2; echo("overwrite", dead);

// A nested scope falls through to the outer binding before its local is set.
ox = 99;
module hoist_scope() { ny = ox; ox = 5; echo("nested", ny, ox); }
hoist_scope();
