# Introduction

The staking contract allows users to create pools with two ERC20 tokens.  Users
can stake the staking_token to earn reward_token based on: time staked, and stake
share of total_staked.

# Data Structure
Each stake pool is stored into a single URef as a byte array [u8; 232] containing information
about the pool:

```
    pub struct StakePool {
        // 0..8
        pub id: u64,
        // 8..16,
        pub last_reward_timestamp: u64,
        // 32 : 48
        pub staking_token: ContractHash,
        // 32 : 80
        pub reward_token: ContractHash,
        // 8  : 88
        pub start_time: u64,
        // 8  : 96
        pub end_time: u64,
        // 8  : 104
        pub precision: u64,
        // 32 : 136
        pub owner: AccountHash,
        // 32 : 168
        pub total_reward: U256,
        // 32 : 200
        pub acc_token_per_share: U256,
        // 32 : 232
        pub total_staked: U256,
    }
```
The UserInfo type is a 64-byte struct as follows:

```
pub struct UserInfo {
    // 32
    pub amount: U256,
    // 32 : 64
    pub reward_debt: U256,
}
```


# Navigation

All of the main functionalities for this contract can be found in /stake.rs.
These functions are added to the runtime as callable entry points in /main.rs.
/pool.rs implements data manipulation functions for contract storage.
The associated arg names for each entry point are found in /constants.rs.
/utils.rs and /address.rs are used as helpers, and /lib.rs implements errors.
/interact_token.rs implements ERC20 token functionality for the contract.

# Deployment

NOTE: linux is recommended for building and deploying

Build the contract:
```
make build-contract

```

Deploy the contract:

```
casper-client put-deploy --node-address http://3.208.91.63:7777 --chain-name casper-test --secret-key \
<PATH_TO_KEY>.pem \--payment-amount 100000000000 \--session-path <path_to_contract_target>\
control_erc20_contract/target/wasm32-unknown-unknown/release/contract.wasm

```

Call 'init' entrypoint via CLI with args for the contract: 

```

 casper-client put-deploy \--node-address http://3.208.91.63:7777 \--chain-name casper-test \ --secret-key <PATH_TO_KEY>.pem \--payment-amount 60000000000 \--session-hash hash-<CONTRACT_HASH> \ --session-entry-point "init" \--session-arg "package-hash:string='contract-package-wasm<CONTRACT_PACKAGE_HASH>'"

```

# Testing

```
make test
```
