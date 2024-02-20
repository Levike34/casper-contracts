#![no_std]
#![no_main]

#[cfg(not(target_arch = "wasm32"))]
compile_error!("target arch should be wasm32: compile with '--target wasm32-unknown-unknown'");

extern crate alloc;

use alloc::string::String;

use casper_contract::{
    contract_api::{account, runtime, runtime::revert, system},
    unwrap_or_revert::UnwrapOrRevert,
};
use casper_types::{
    account::AccountHash, api_error::ApiError, runtime_args, ContractHash, Key, PublicKey,
    RuntimeArgs, U256, U512,
};
enum Contract {
    Lock,
    TransferLock,
    ExtendLock,
    Claim,
    Approve,
}

#[no_mangle]
pub extern "C" fn call() {
    let entry_point: u8 = runtime::get_named_arg("entry_point");
    let contract_hash: String = runtime::get_named_arg("contract_hash");

    let contract = ContractHash::from_formatted_str(&contract_hash).unwrap();

    match entry_point {
        0 => {
            let token_hash: String = runtime::get_named_arg("token_hash");
            let amount_to_transfer: U256 = runtime::get_named_arg("amount_to_transfer");
            let cliff_durtime: u64 = runtime::get_named_arg("cliff_durtime");
            let time_between_locks: u64 = runtime::get_named_arg("time_between_locks");
            let num_locks: u32 = runtime::get_named_arg("num_locks");

            // let recipient: String = runtime::get_named_arg("recipient");
            // let recip: AccountHash = AccountHash::from_formatted_str(&recipient).unwrap();
            // let amount: U512 = runtime::get_named_arg("amount");

            // if amount < U512::from(3_000_000_000_u128) {
            //     revert(ApiError::InvalidArgument)
            // }

            // let main_purse = casper_contract::contract_api::account::get_main_purse();
            // system::transfer_from_purse_to_account(main_purse, recip, amount, None)
            //     .unwrap_or_revert();

            runtime::call_contract(
                contract,
                "lock",
                runtime_args! {
                    "token-hash" => token_hash,
                    "cliff_amount" => amount_to_transfer,
                    "cliff_durtime" => cliff_durtime,
                    "time_between_locks" => time_between_locks,
                    "num_locks" => num_locks,
                },
            )
        }
        1 => {
            let new_owner: PublicKey = runtime::get_named_arg("new_owner");
            let lock_id: u64 = runtime::get_named_arg("lock_id");

            runtime::call_contract(
                contract,
                "transfer_lock",
                runtime_args! {
                    "lock_id" => lock_id,
                    "new_owner" => new_owner
                },
            )
        }
        2 => {
            let new_release_time: u64 = runtime::get_named_arg("new_release_time");
            let lock_id: u64 = runtime::get_named_arg("lock_id");
            let index: u32 = runtime::get_named_arg("index");

            runtime::call_contract(
                contract,
                "extend_lock",
                runtime_args! {
                    "lock_id" => lock_id,
                    "new_release_time" => new_release_time,
                    "index" => index
                },
            )
        }
        3 => {
            let lock_id: u64 = runtime::get_named_arg("lock_id");

            runtime::call_contract(
                contract,
                "claim",
                runtime_args! {
                    "lock_id" => lock_id
                },
            )
        }
        4 => {
            let token_hash: String = runtime::get_named_arg("token_hash");
            let spender: String = runtime::get_named_arg("spender");
            let amount: U256 = runtime::get_named_arg("amount");

            runtime::call_contract(
                ContractHash::from_formatted_str(&token_hash).unwrap(),
                "approve",
                runtime_args! {
                    "spender" => Key::from_formatted_str(&spender).unwrap(),
                    "amount" => amount
                },
            )
        }
        _ => {}
    }
}
