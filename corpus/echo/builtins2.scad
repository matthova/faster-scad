echo(is_undef(undef), is_num(3), is_string("x"), is_list([1]), is_bool(true));
echo(lookup(2.5, [[0,0],[1,10],[2,20],[3,30]]));
echo(search(3, [1,2,3,4,5]));
echo(search("b", "abcabc"));
echo(search([1,3], [1,2,3]));
echo(search("foo", "food"));
echo(search(9, [1,2,3]));
echo(search("a", "abcabc", 0));
