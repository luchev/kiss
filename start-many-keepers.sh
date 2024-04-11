#!/bin/bash

for i in `seq 1 $1`;
do
    just run peer$i &
done

echo "Done"

wait
