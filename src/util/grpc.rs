pub mod immudb_grpc {
    tonic::include_proto!("immudb.schema");
}

pub mod kiss_grpc {
    tonic::include_proto!("kiss_grpc");
}

pub mod kademlia_grpc {
    tonic::include_proto!("kademlia_grpc");
}
