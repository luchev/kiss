# (Abstract) Thesis proposal

## Background

There are a lot of decentralized storage systems, which introduce mechanics to fend off malicious peers, which aim to disrupt the system by hampering the communication between peers.
Examples for such attacks are:

- Eclipse attacks, where a malicious peer attempts to isolate a peer from the rest of the network.
- Sybil attacks, where a malicious peer attempts to create multiple identities in the network, in order to gain more power.
- DDoS attacks, where a malicious peer attempts to overload a peer with requests, in order to disrupt its normal operation.

In this project we focus on attacks that disrupt the storage guarantees of the system, i.e., modification/deletion of files.
Such attacks are possible because the peers in the network are usually not controlled by an authority.
There is usually no major incentive for peers to behave honestly or punishment for misbehaving.

This proposal builds on top of an existing decentralized storage system using verification.
The system so far has the following properties and functionalities:

- Based on S/Kademlia
- Replication of files
- Persistent information about the state of the network using an immutable database
- Continuous verification of file integrity using Proofs of Retrievability
- Proof-of-Concept reward/punishment system for peers

## Goal

The Goal of this project is to enhance the existing system and build on top of it.

### Decentralize the Immutable Database

The immutable database being used currently is ImmuDB, ran in a centralized manner.
To achieve a decentralized system, we need to decentralize the immutable database.
We can do this in one of multiple possible ways:

- Migrate to an immutable decentralized database (perhaps ImmuDB has this feature)
- Use a blockchain to store the immutable database
- Create a custom solution, where the database is represented as files stored in the network

### Improve the Proofs of Retrievability

Currently, we are using a proof of retrievability protocol, which requires the client (each verifier)
to store $O(\sqrt{N})$ additional metadata for each file, where $N$ is the size of the file.
There are more efficient protocols, which require $O(\log{N})$ additional metadata or even constant additional metadata.
Some protocols also allow public verifiability, which means that the verifier does not need to store any additional metadata, as it can be stored in an encrypted manner in the file itself.
We would like to investigate these protocols and improve the current implementation.

### Improve the Reward/Punishment System

The existing Reward/Punishment system is very simple and relies on high levels of trust in the system, as well as 
a small number of malicious peers.
We would like to improve the system by introducing a token-based system, where peers are rewarded for storing files and punished for misbehaving.
These tokens/reputation points can be stored:

- In the system itself (as files) or
- As coins from a blockchain

The ideal solution would be to use coins from a blockchain, that are traded on exchanges.
This way, there will be monetary incentive for peers to behave honestly.
When a new peer wants to join the network, they would have to purchase tokens from the blockchain and stake them in the network.
When someone wants to store a file, they would have to pay for the storage using their own tokens.
If a peer misbehaves, their tokens will be slashed — used as compensation for the user/peer who lost their file.
On the other hand, if a peer behaves and stores files, they will be rewarded with more tokens — the tokens that were paid by the client.
If a peer's tokens drop below a certain threshold, they will be kicked out of the network.
Verifiers will be rewarded tokens if they discover a malicious peer, which means that during the verification process, they discover a peer that is not storing a file as promised.

Some possible blockchain solutions are:

- Polkadot (using [https://substrate.io/](https://substrate.io/)) or
- Solana ([https://solana.com/developers](https://solana.com/developers)) or
- Near protocol ([https://near.org/](https://near.org/))

### Other
The system has fully decentralized storage and verification, however, it is not rigorously tested against different malicious peers behaviors.
We would like to experiment with different types of possible attacks and further improve the system to be able to withstand them.
And lastly — any other potential improvements that might be found during the process of working on the above features.
