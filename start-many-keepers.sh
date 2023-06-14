#!/bin/bash

for i in `seq 1 $1`;
do
    KISS_grpc_port=0 KISS_swarm_port=0 ./target/release/keeper &
done

echo "Done"

wait

