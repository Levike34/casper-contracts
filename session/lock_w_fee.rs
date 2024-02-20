#![no_std]
#![no_main]

#[cfg(not(target_arch = "wasm32"))]
compile_error!("target arch should be wasm32: compile with '--target wasm32-unknown-unknown'");

extern crate alloc;

use alloc::string::String;

use casper_contract::{
    contract_api::{runtime, runtime::revert, system},
    unwrap_or_revert::UnwrapOrRevert,
};
use casper_types::{
    account::AccountHash, api_error::ApiError, runtime_args, ContractHash, RuntimeArgs, U256, U512,
};
#[no_mangle]
pub extern "C" fn call() {
    let token_hash: String = runtime::get_named_arg("token_hash");
    let amount_to_transfer: U256 = runtime::get_named_arg("amount_to_transfer");
    let cliff_durtime: u64 = runtime::get_named_arg("cliff_durtime");
    let time_between_locks: u64 = runtime::get_named_arg("time_between_locks");
    let num_locks: u32 = runtime::get_named_arg("num_locks");

    let recipient: String = runtime::get_named_arg("recipient");
    let recip: AccountHash = AccountHash::from_formatted_str(&recipient).unwrap();
    let amount: U512 = runtime::get_named_arg("amount");

    if amount < U512::from(3_000_000_000_u128) {
        revert(ApiError::InvalidArgument)
    }

    let main_purse = casper_contract::contract_api::account::get_main_purse();
    system::transfer_from_purse_to_account(main_purse, recip, amount, None).unwrap_or_revert();

    runtime::call_contract(
        ContractHash::from_formatted_str(
            "contract-2bb07bf141774dc3ad1419c2e447abfff519ee1272bfe8b4a5c19c112113ce72",
        )
        .unwrap(),
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
