# (Abstract) Project proposal for the 2023 spring semester

The idea is to create a distributed storage system based on a structured p2p network. The main goals of the system are to ensure durable storage by replication. Also, try to prevent some of the more popular attacks by using a blockchain network, which keeps track of peers’ reputations.

## Specifications

* 2 Kind of nodes:
    * Keeper: Stores files, provides storage and bandwidth
    * Validator: Checks Keepers’ contracts for storing files
        * In order to avoid Malicious validators, the files a validator is responsible for will be rotated
* Data redundancy—data will be replicated to 3 nodes
    * Validator nodes will be responsible for checking this replication invariant. If a node loses the file, the Validator will pick another node to store the file
* Keeper nodes will be awarded tokens whenever a contract they made is verified
    * Tokens will be kept on a blockchain to avoid malicious peers lying about their tokens
* Ideas for token awards by Verifiers:
    * Longer storage period → More tokens
    * Faster download → More tokens
    * More popular file → More tokens (probably a bad idea because nodes will prefer popular files and drop less popular ones)
        * This can be mitigated by increasing the rewards for unpopular files. This will cause files to oscillate between being “popular” and “unpopular”, which pay out high rewards, but files that are neither popular nor unpopular (in the middle) will be less profitable.
* Clients that want to store a file will have to “pay” for that file storage with their own tokens
    * Ideally, tokens can be “purchased” using tokens from other chains
    * (Optional) have a more practical method of payment
* File storage contracts will be timed. After the specified time, the file will be removed
* Files will be stored under a hashed key
* File indexing/discovery will happen separately from the network

## Normal operational flow

* We start with at least:
    * 1 Keeper node
    * 1 Verifier node
    * 1 Client who wants to store a file, which is 100MB
* The Client contacts the Verifier with a request to store a file with size 100MB, for 10 days
* The Verifier proposes a contract, which will cost the Client X number of tokens to store the file for that period
* The Client accepts
* The Verifier takes the file and contacts Keeper nodes, offering them a contract to store the given file for 10 days for Y number of tokens
* The Verifier distributes the file to the Keeper nodes that accept the contract
* The Verifier creates a hash of the file and verifies that the Keeper nodes have the file by asking them to send the Verifier the hash of the file. This check occurs regularly
* The Verifier chooses another 2 Verifiers (based on proximity in the ID space), which should also hold the hash of the file
* The Client is informed that the contract is complete and is given the IDs of the Verifiers that know where the file is stored.
* If the Client wants to retrieve the file, they contact the Verifiers, which forward the request to the Keeper nodes
* If the Client wishes to store the file for longer, they need to establish a new contract before the 10 days period ends

## Technical details

* Language of choice: Rust (preferred due to style and speed) or Go (more libraries)
* Build on top of
    * Polkadot (using [https://substrate.io/](https://substrate.io/)) or
    * Solana ([https://solana.com/developers](https://solana.com/developers)) or
    * Near protocol ([https://near.org/](https://near.org/))
* Go:
    * [https://github.com/algorand](https://github.com/algorand)
    * 

## Security

* Preventing peers from hurting the durable storage guarantee: Peers will be required to “stake” their tokens in order to store files. File integrity will be checked randomly and if the file storage contract isn’t obeyed, the peer’s tokens will be slashed
* Preventing Sybil attacks: Peers joining the network need to solve a crypto puzzle before joining. Also, the previous point
* Eclipse attacks: The reason we are choosing a structured p2p network
* DDoS: _Unclear_

## Privacy

* (Optional) Files will be stored encrypted
* (Optional) Access to files will be allowed only for clients, which have an access token (key)
* File editing/deletion will be allowed only for clients, which have a certificate (key)

## Ideas and alternatives

* Use hierarchical naming and allow Keeper nodes to have multiple branches of the storage tree. This way, Keeper nodes can select what files they want to store. This would enable replicating popular files more by making them more desirable, as they will reward more tokens.
    * We have to balance the rewards [least desirable --- average --- most desirable]
    * The token rewards should be high for files that nobody wants to store and that everyone wants to store, in order to make sure there is enough incentive to keep files in the system even if they are accessed rarely
    * Choosing which peer to connect to will be more complicated, but we can make use of locality and increase speed for file retrieval (building a sort of p2p CDN)
