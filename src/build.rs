fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/immudb.proto")?;
    tonic_build::compile_protos("proto/kiss.proto")?;
    tonic_build::compile_protos("proto/kademlia.proto")?;
    Ok(())
}
