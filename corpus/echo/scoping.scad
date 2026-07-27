a = 10;
function f() = a;
module m() { a = 20; echo("m:", f(), a); }
m();
echo("global:", f());
function g(x) = x + a;
module n(a) { echo("n:", g(1)); }
n(99);
lit = function() a;
module p() { a = 77; echo("lit:", lit()); }
p();
$fn = 8;
module ring2() { echo("fn:", $fn); }
module setfn() { $fn = 16; ring2(); }
setfn();
ring2();
