pub mod immudb_grpc {
    tonic::include_proto!("immudb.schema");
}

pub mod keeper_grpc {
    tonic::include_proto!("keeper_grpc");
}

pub mod verifier_grpc {
    tonic::include_proto!("verifier_grpc");
}

pub mod kademlia_grpc {
    tonic::include_proto!("kademlia_grpc");
}
