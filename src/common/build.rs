fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/immudb.proto")?;
    tonic_build::compile_protos("proto/keeper.proto")?;
    tonic_build::compile_protos("proto/verifier.proto")?;
    tonic_build::compile_protos("proto/kademlia.proto")?;
    Ok(())
}
