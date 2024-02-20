# Introduction

The vesting contract's purpose is for users to send tokens to the smart contract for it to lock
and hold for the specified time period, and releasing those tokens to the recipient according to
the time schedule set by the user initially.  Each lock has 3 basic functionalities: transfer, extend, and unlock.  transfer changes the recipient of the unlocked tokens to another user.  extend increases the lock time of the lock.  unlock withdraws tokens based on the amount of time passed, relative to the lock schedules.  

# Navigation

All of the main functionalities for this contract can be found in /vest.rs.
These functions are added to the runtime as callable entry points in /main.rs.
The associated arg names for each entry point are found in /constants.rs.
/utils.rs and /address.rs are used as helpers, and /lib.rs implements errors.
/interact_token.rs implements ERC20 token functionality for the vesting contract.

# Deployment

NOTE: linux is recommended for building and deploying

Build the contract:
```
cargo +nightly build --release --target wasm32-unknown-unknown

```

Deploy the contract:

```
casper-client put-deploy --node-address http://3.208.91.63:7777 --chain-name casper-test --secret-key /<PATH_TO_KEY>.pem --payment-amount 100000000000 --session-path <path_to_contract_target>/control_erc20_contract/target/wasm32-unknown-unknown/release/contract.wasm

```

Call 'init' entrypoint via CLI with args for the contract: 

```

 casper-client put-deploy --node-address http://3.208.91.63:7777 --chain-name casper-test --secret-key /<PATH_TO_KEY>.pem --payment-amount 60000000000 --session-hash hash-<CONTRACT_HASH> --session-entry-point "init" --session-arg "scontract-hash:string='contract-package-wasm<CONTRACT_PACKAGE_HASH>'" --session-arg "token-hash:string='contract-b02ec9fe439a945bcc0cc4a786f22fab7ae41829e10ea029e6f82af1b3833b60'"

```

# Testing

Test tokens can be easily deployed using the casper-erc20-js-interface and CLI.