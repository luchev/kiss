set shell := ["bash", "-uc"]

run env:
    ENV={{env}} RUST_LOG=info cargo run

debug env:
    ENV={{env}} RUST_LOG=debug cargo run

put data:
    grpcurl \
    -plaintext \
    -import-path proto \
    -proto kiss.proto \
    -d "{\"name\": \"key1\", \"content\": \"`echo {{data}} | perl -pe 'chomp if eof' | base64`\", \"ttl\": \"1200\"}" \
    "[::1]:2000" \
    kiss_grpc.KissService/Store

get uuid:
    grpcurl \
    -plaintext \
    -import-path proto \
    -proto kiss.proto \
    -d "{\"name\": \"{{uuid}}\"}" \
    '[::1]:2000' \
    kiss_grpc.KissService/Retrieve

verify file_uuid:
    grpcurl \
    -plaintext \
    -import-path proto \
    -proto kiss.proto \
    -d "{\"file_uuid\": \"{{file_uuid}}\"}" \
    "[::1]:2000" \
    kiss_grpc.KissService/VerifyFile

providers uuid:
    grpcurl \
    -plaintext \
    -import-path proto \
    -proto kiss.proto \
    -d "{\"name\": \"{{uuid}}\"}" \
    "[::1]:2000" \
    kiss_grpc.KissService/GetProviders

put-to data peer_uuid:
    grpcurl \
    -plaintext \
    -import-path proto \
    -proto kiss.proto \
    -d "{\"content\": \"`echo {{data}} | base64`\", \"ttl\": \"1200\", \"peer_uuids\": [\"{{peer_uuid}}\"] }" \
    "[::1]:2000" \
    kiss_grpc.KissService/PutTo

get-closest uuid:
    grpcurl \
    -plaintext \
    -import-path proto \
    -proto kiss.proto \
    -d "{\"uuid\": \"{{uuid}}\"}" \
    "[::1]:2000" \
    kiss_grpc.KissService/GetClosestPeers

test:
    cargo test

test-nocap:
    RUST_LOG=debug cargo test -- --nocapture

release:
    cargo build --release

build: release

run-many count: release
    for i in $(seq 1 {{count}}); do \
        RUST_LOG=info ENV=peer$i ./target/release/kiss &>logs/peer$i.log & \
    done

run-many-no-build count:
    for i in $(seq 1 {{count}}); do \
        RUST_LOG=info ENV=peer$i ./target/release/kiss &>logs/peer$i.log & \
    done

kill-all:
    pkill -f kiss

run-random-keeper:
    KISS_grpc_port=0 KISS_swarm_port=0 RUST_LOG=info ./target/release/keeper

thesis:
    cd docs/Thesis/ && tectonic -X build

create-db:
    docker run -d --name immudb -p 3322:3322 codenotary/immudb:latest

remove-db:
    docker rm -f immudb

recreate-db: remove-db create-db

clean-data:
    rm -rf data/*

clean-logs:
    rm -rf logs/*

clean: recreate-db clean-data clean-logs kill-all

put-many numbytes:
    openssl rand -base64 {{numbytes}} > /tmp/bytes
    grpcurl \
    -plaintext \
    -import-path proto \
    -proto kiss.proto \
    -d "{\"name\": \"key1\", \"content\": \"`cat /tmp/bytes | perl -pe 'chomp if eof' | base64`\", \"ttl\": \"1200\"}" \
    "[::1]:2000" \
    kiss_grpc.KissService/Store

put-many-times numbytes times:
    for i in $(seq 1 {{times}}); do \
        echo -n '{"name": "key1", "ttl": 1200, "content": "' > /tmp/grpcurl.json; \
        openssl rand -base64 {{numbytes}} | perl -pe 'chomp if eof' | base64 | tr -d "\n" >> /tmp/grpcurl.json; \
        echo -n '"}' >> /tmp/grpcurl.json; \
        grpcurl \
        -plaintext \
        -import-path proto \
        -proto kiss.proto \
        -d @ \
        "[::1]:2000" \
        kiss_grpc.KissService/Store < /tmp/grpcurl.json; \
    done
