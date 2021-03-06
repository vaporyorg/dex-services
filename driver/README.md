# Running pepper with snarks from this repo

## Overview: Driver

This is the software for the driver of a snapp. The driver is responsible for pushing new states into the snapp base. For this it gets all necessary information from the database and the smart-contract and then starts to calculate new states and pushes them into the snapp. New states are calculated based on deposit, withdrawal and order inputs. 

## Prerequisites:

## Setup:

The driver interacts with ganache-cli (ethereum blockchain) and a GraphNode postgres database. It is the easiest to simply use the docker compose of the parent file to get everything running up and communicating. 

- do not use any authentication for the database.


Install rust & cargo


Deploy a [SnappBase Contract](https://github.com/gnosis/dex-contracts):

Run a local test network
```bash
ganache-cli -d
```

Go into the dex-contracts repo and run
```bash
truffle migrate 
```

Push some data into the data base as tests. Usually the event_listener would do that for us.

Running 

```sh
cargo run 

```
will start a listener for the data base and tries to push new states after applying deposits (Here it fails as the depositHash in the smart contract is just bytes32(0)".

