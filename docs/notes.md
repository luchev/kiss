1. List all things to evaluate but do only a few

2. Check if performance scales linearly with more peers
3. Verify the system works - can upload and download files
6. What is the bottleneck in the system?

4. What are the different malicious peers - does the system detect them, and how does it fight them off?
5. How much is my code contributing on top of Kademlia?
7. Try adding 100k files and measure how long it takes
8. Try adding large files (100mb+)
9. What is the hotspot in terms of network/storage for 100k files?
10. Write a small paragraph on why rust in the architecture section to justify why is the architecture as it is
11. UML Diagram on the architecture of the whole app - and why it is designed this way.
12. read Rkademlia

Starting many processes:

1 process: 2MB
5 processes: 10MB
100 processes: 200MB
1000 processes: 2GB
CPU is idle since no requests come in
(2 screenshots)

storing 1k files
10.8s 1 peer 2% CPU
12.4s 3 peers 2% CPU 3% receiver
13s 10 peers 2% CPU
16s 100 peers
verifier: 5% CPU

storing 100k files:
20m20s
