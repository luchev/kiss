run env:
    ENV={{env}} RUST_LOG=info cargo run

debug env:
    ENV={{env}} RUST_LOG=debug cargo run

put data:
    grpcurl \
    -plaintext \
    -import-path proto \
    -proto kiss.proto \
    -d "{\"name\": \"key1\", \"content\": \"`echo {{data}} | base64`\", \"ttl\": \"1200\"}" \
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

keeper1: release-keeper
    ENV=peer1 RUST_LOG=info ./target/release/keeper

keeper2: release-keeper
    ENV=peer2 RUST_LOG=info ./target/release/keeper

keeper: debug-common
    ENV=peer1 RUST_LOG=info cargo run --package keeper

verifier: debug-common
    RUST_LOG=info cargo run --package verifier

cargo-verifier:
    RUST_LOG=info cargo run --package verifier

cargo-keeper:
    RUST_LOG=info cargo run --package keeper

release-keeper:
    cargo build --release --package keeper

release-verifier:
    cargo build --release --package verifier

release-common:
    cargo build --release --package common

debug-keeper:
    cargo build --package keeper

debug-verifier:
    cargo build --package verifier

debug-common:
    cargo build --package common

run-random-keeper:
    KISS_grpc_port=0 KISS_swarm_port=0 RUST_LOG=info ./target/release/keeper
