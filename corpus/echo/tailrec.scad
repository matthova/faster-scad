function sum(n, acc=0) = n==0 ? acc : sum(n-1, acc+n);
echo(sum(50000));
function count_down(n) = n<=0 ? "done" : let(m=n-1) count_down(m);
echo(count_down(20000));
function gcd(a, b) = b==0 ? a : gcd(b, a%b);
echo(gcd(1071, 462));
