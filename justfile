keeper:
    RUST_LOG=info cargo run --package keeper

keeper2:
    ENV=peer2 RUST_LOG=info cargo run --package keeper

verifier:
    RUST_LOG=info cargo run --package verifier
