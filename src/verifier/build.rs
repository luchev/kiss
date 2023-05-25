fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../common/proto/immudb.proto")?;
    tonic_build::compile_protos("../common/proto/verifier.proto")?;
    tonic_build::compile_protos("../common/proto/keeper.proto")?;
   Ok(())
}
