// Eval-bound: heavy pure computation (tail-recursive Collatz over a big range),
// tiny geometry output. Stresses the interpreter, not the kernel.
function collatz(n, acc=0) =
    n <= 1 ? acc : collatz(n % 2 == 0 ? n/2 : 3*n + 1, acc + 1);
steps = [for (i = [1:1:15000]) collatz(i)];
m = max(steps);
// a tiny cube whose size depends on the whole computation
cube(m / 100);
