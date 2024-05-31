# ABSE-DAG-Rider
Apply ABSE to DAG-Dider.

## How to run
Run

```Bash
Cargo build
```

(add --release if you want to run in release mode) and it will generate client and node in the corresponding directory of the target. Run

```Bash
./client -h
```
```Bash
./node -h
```
```Bash
./node generate -h
```
to view parameter definitions.

The subcommand `generate` provide a way to generate multiple files for multi processes.
It also helps to generate a bunch of bash scripts to run all the nodes.

Example:

Generate relevant configuration files in the current directory (batch size: 10, channel capacity： 1000， 16 processes, 0 fauties):
```Bash
./node generate --batch_size 10 --channel_capacity 1000 --node_count 16 --faulty_count 0 --faulty_type 0
```

This generates the committee.json and run_node.sh configuration files. 
Then run:
```Bash
bash run_node.sh
```
to start all nodes.

You also need to start the clients to send transactions to the node, for example:
```Bash
./client --TRANSACTION_COUNT 100 --TX_SIZE 40 127.0.0.1:8124 
```
We implement the clients to send packets to processes at a certain rate (as described in the paper) by writing the bash script manually (i.e., sending a certain number of transactions at regular intervals).
Note that the default ports for the nodes start at 127.0.0.1:8123, where each process occupies three consecutive ports, the second port is used to receive transactions, and you can follow this logic to find the port number of the process you need.


## Some notes related to the code

Note: Currently, throughput and latency do not support automatic statistics and may need to be calculated manually.

Note: It is possible to set RUST_LOG=DEBUG for the node in run_nodes.sh to print the node's DAG graph as well as transaction's detail on the console every round. However, this affects system performance quite a bit and is only recommended when verifying that the system is functioning correctly.

Note: The main DAG-Dider code was taken from https://github.com/Shendor/dag-rider with modifications.

V0.2 additional features are as follows:

1. Increase the scalability of the system

2. Support to change the setting parameters through the command line and add the function of automatic generation of scripts.

3. Retain the error handling and task management functions while avoiding the main thread being blocked.

4. Added node simulation of faulty nodes related components

V0.3 

Logging is now accurate to the ms level, making it easier to calculate throughput and latency

V0.4

Now that the system is able to simulate adversaries that behaves as in the paper, try adding the "--faulty_type 2" parameter in generate.

