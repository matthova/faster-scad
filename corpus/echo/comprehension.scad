echo([for(i=[0:4]) i*i]);
echo([for(i=[0:5]) if(i%2==0) i]);
echo([for(i=[1:3]) let(sq=i*i) sq]);
echo([for(i=[0:2], j=[0:2]) i*10+j]);
echo([each [1,2], each [3,4]]);
echo([for(i=[0:3]) if(i>1) i else -i]);
