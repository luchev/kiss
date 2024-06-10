# KISS

A decentralized storage system based on Kademlia, augmented with audits,
based on a Proof of Retrievability protocol,
and a reputation system using Immudb to ensure data integrity and availability.

## Features

- **Replicated storage**: Data is stored across multiple nodes in a Kademlia network.
- **Data Integrity**: Data is stored with a metadata file, allowing other peers to perform audits.
- **Reputation System**: Nodes are given ratings based on their performance and reliability.
- **Proof of Retrievability**: Data can be audited to ensure it is still available using a
    PoR protocol based on [Dynamic proofs of retrievability with low server storage](https://github.com/dsroche/la-por).

## Build

### Prerequisites

- Cargo/Rust 1.74.0-nightly
- Docker
- [Just](https://github.com/casey/just)
- openssl
- grpcurl
- protoc

### Build

The build action is not necessary as the project uses Just scripts to run the app
and runs the build action automatically when necessary.
It is possible to build the project using the following command:

```
just build
```

### Thesis (LaTeX)

Building the thesis requires [tectonic](https://github.com/tectonic-typesetting/tectonic)
and the [Rust listings](https://github.com/denki/listings-rust).

```
just thesis
```

## Usage


### Preparing the environment

Before running the project we must ensure the state of the system is clean,
i.e., logs, data, and old builds are removed, Immudb is running.
We can use the script:

```
just clean
```

This script also tries to kill existing instances of the app, so it may show an error that it
couldn't find any instances to kill.

### Running the project

Running the project can be done with the following command:

```
just run <config>
```

Where `<config>` can be any of the names of the configs under `config/`.

The rest of the just scripts expect `base` and `dev` to be running.
```
just run base
just run dev
```

### Storing and retrieving files

Storing files can be done with the following command:

```
just put "any data that you want to store"
```

This will return a UUID such as `00000000-0000-0000-0000-000000000000`.

Now that UUID can be used to retrieve the data:

```
just get <UUID>
```

### Scripts many instances/files

When running benchmarks it could be useful to run many instances of the app.
```
just run-many <number of instances>
```

It is also possible to store many files at once with the following command:

```
just put-bytes-times <numbytes> <times>
```

This will store `<times>` random files of size `<numbytes>`.

### Tests

Running the unit tests is done using:

```
just test
```

Running the benchmarks is done using:

```
just bench
```
