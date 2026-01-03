clear
rm -f nohup.out 
#rm -f data/*
rm -f logs/*
kill -9 `ps axuww|grep highres_pair_c0|grep -v "grep"|awk '{print$2}'`
#sleep 5
nohup cargo run &
