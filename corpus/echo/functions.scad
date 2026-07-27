f = function(x) x*x;
echo(f(5));
g = function(a,b) a+b;
echo(g(3,4));
echo(is_function(f), is_function(5));
echo([for(i=[1:4]) f(i)]);
h = function(n) n<=1 ? 1 : n*h(n-1);
echo(h(5));
