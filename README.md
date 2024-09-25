# Withdraw commission tool

There is a pseudo-bug in the Cosmos SDK commands where both validator `MsgWithdrawDelegateRewards` and `MsgWithdrawValidatorCommission` are combined in a single transaction when using the command line tool. This is troublesome because if the balance of delegate rewards happens to be zero, the transaction will fail, preventing the validator from withdrwaing their commission. This is a simple utility that submits a transaction containing *only* the `MsgWithdrawValidatorCommission` message.

## Installation

### Prerequisites

- Have `cargo` installed

### Steps

Right now you have to build from source as there is no release artifact in this repository:

1. Clone this repository and `cd` into it

2. Run the following command to install the binary to your PATH

```bash
cargo install --path .
```

## Usage

```bash
withdraw-commission --private-key-path <PATH TO VALIDATOR SIGNING KEY>
```

Optional arguments:

```bash
# Default values shown
withdraw-commision \
    --chain-id sommelier-3 \
    --rpc-url https://sommelier-rpc.polkachu.com:443 \
    --grpc-url https://sommelier-grpc.polkachu.com:14190 \
    --denom usomm \
    --timeout-height 0
    --signing-key-path <YOUR KEY PATH>
```
