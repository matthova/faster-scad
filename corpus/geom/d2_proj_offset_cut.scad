linear_extrude(2) offset(delta=2) difference(){ square(30,center=true); projection(cut=true) rotate([0,0,30]) cube([12,12,20],center=true); }
