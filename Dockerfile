FROM rust

RUN apt update && apt install -y vim protobuf-compiler htop
RUN rustup install nightly-2023-04-03
RUN rustup default nightly-2023-04-03
