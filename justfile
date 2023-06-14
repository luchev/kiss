keeper1:
    ENV=peer1 RUST_LOG=info ./target/release/keeper

keeper2:
    ENV=peer2 RUST_LOG=info ./target/release/keeper

verifier:
    RUST_LOG=info ./target/release/verifier

cargo-verifier:
    RUST_LOG=info cargo run --package verifier

cargo-keeper:
    RUST_LOG=info cargo run --package keeper

build-keeper:
    cargo build --release --package keeper

build-verifier:
    cargo build --release --package verifier

run-random-keeper:
    KISS_grpc_port=0 KISS_swarm_port=0 RUST_LOG=info ./target/release/keeper
