# Introduction

The vesting contract's purpose is for users to send tokens to the smart contract for it to lock
and hold for the specified time period, and releasing those tokens to the recipient according to
the time schedule set by the user initially.  Each lock has 3 basic functionalities: transfer, extend, and unlock.  transfer changes the recipient of the unlocked tokens to another user.  extend increases the lock time of the lock.  unlock withdraws tokens based on the amount of time passed, relative to the lock schedules.  

# Data Structure
Each lock is stored into a single URef as a byte array containing information about the lock:

```
VestInfo {
    // 8
    id: u64,
    // 8 + 8 = 16
    lock_time: u64,
    // 8 + 8 + 32 = 48
    recipient: [u8; 32],
    // 8 + 8 + 32 + 32 = 80
    token_hash: [u8; 32],
    // 8 + 8 + 32 + 32 + (40 * number of schedules)
    schedules: Vec<LockSchedule>
}
```
ex.) https://testnet.cspr.live/uref/dictionary-7f2ddcc039a9ca64f29741053e103b02f94bb5c80a94af1b82c7ed571006c31e <br></br>
The LockSchedule type is a 40-byte struct as follows:

```
LockSchedule {
    // 8
    release: u64,
    // 8 + 32 = 40
    amount: U256
}
```

***NOTE: All u64 and U256 values are converted to and stored as Little Endian byte arrays*** <br></br>
***NOTE2: All lock IDs are from the global counter URef KEY_NAME_INDEX***

All active user locks can be accessed by KEY_NAME = caller's AccountHash; the result is a Vec<u64> where
each element represents an active VestInfo that account owns.

# Security
The function caller_is_recipient() takes the lock_id as a parameter and uses it
to create the dictionary key from the current index.  If the key exists,
the function will compare the 32 bytes in VestInfo from 16..48 to see if the caller
is the owner of that lock. transferLock(), extendLock(), and claim() all require this
function first. claim() loops through VestInfo.schedules and if the timestamp is greater
than that schedule's release, the amount is set to 0 and that amount is transferred during
the call.  Next time it will still loop through that schedule and add 0 to the total transfer
amount.

make_key_by_id(lock_id: u64) is the function responsible for creating dictionary keys to access each VestInfo by id. This
function takes the lock_id and turns it into a string, and appends it to the end of the first 15 characters of the ContractPackageHash to the end and writes it to the dictionary. Every function calls this for 
verification as outlined above.

# Function Effect on Storage
transferLock(lock_id: u64, new_owner: AccountHash) -> Replaces 32 bytes at VestInfo.recipient with new_owner. <br></br>
claim(lock_id: u64) -> Loops each lock in VestInfo.schedules and replaces bytes 8..40 with 0 if the timestamp is greater than the release (bytes 0..8) <br></br>
extendLock(lock_id: u64, index: u32, new_release: u64) -> replaces VestInfo.schedules[index] (bytes 0..8) with the new_release if it is greater than the old_release. <br></br>

lock -> Writes a new VestInfo at the URef by the global index and increments the global index by +1 for the next lock.

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
make prepare
make build-contract

```

Deploy the contract:

```
casper-client put-deploy --node-address http://3.208.91.63:7777 \
--chain-name casper-test \
--secret-key <PATH_TO_KEY>.pem \
--payment-amount 120000000000 \
--session-path <path_to_contract_target>
```

Call 'init' entrypoint via CLI with args for the contract: 

```

 casper-client put-deploy --node-address http://3.208.91.63:7777 \
 --chain-name casper-test \ 
 --secret-key <PATH_TO_KEY>.pem \
 --payment-amount 60000000000 \
 --session-hash hash-<CONTRACT_HASH> \ 
 --session-entry-point "init" \
 --session-arg "package-hash:string='contract-package-wasm<CONTRACT_PACKAGE_HASH>'"

```

# Testing

Test tokens can be easily deployed using the casper-erc20-js-interface and CLI.
