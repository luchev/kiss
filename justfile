run:
    RUST_LOG=info cargo run

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
