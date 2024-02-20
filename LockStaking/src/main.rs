#![no_std]
#![no_main]
#[macro_use]
// #[cfg(not(target_arch = "wasm32"))]
// compile_error!("target arch should be wasm32: compile with '--target wasm32-unknown-unknown'");

// We need to explicitly import the std alloc crate and `alloc::string::String` as we're in a
// `no_std` environment.
extern crate alloc;

use alloc::{collections::BTreeMap, string::String};

use casper_contract::contract_api::{runtime, storage};
use casper_types::{
    account::AccountHash,
    contracts::{EntryPoint, EntryPointAccess, EntryPointType, EntryPoints},
    CLType, CLTyped, ContractHash, ContractPackageHash, Key, Parameter, U256,
};

use lock_staking::constants::{
    ARG_NAME_AMOUNT, ARG_NAME_END_TIME, ARG_NAME_ERC20_SELFCONTRACT_HASH, ARG_NAME_NEW_ADMIN,
    ARG_NAME_POOL_ID, ARG_NAME_PRECISION, ARG_NAME_REWARD_TOKEN, ARG_NAME_STAKING_TOKEN,
    ARG_NAME_START_TIME, ARG_NAME_TOKEN_HASH, ARG_NAME_TOTAL_REWARD, CONTRACT_HASH, CONTRACT_NAME,
    CONTRACT_VERSION, ENTRYPOINT_NAME_ADD_POOL, ENTRYPOINT_NAME_DEPOSIT,
    ENTRYPOINT_NAME_EMERGENCY_WITHDRAW, ENTRYPOINT_NAME_INIT, ENTRYPOINT_NAME_SAVE_ME,
    ENTRYPOINT_NAME_SET_ADMIN, ENTRYPOINT_NAME_STOP_REWARD, ENTRYPOINT_NAME_WITHDRAW,
    KEY_NAME_INITIALIZED, VESTOR_PACKAGE_NAME, VESTOR_UREF_NAME,
};
use lock_staking::{constants::KEY_NAME_ADMIN, StakeContract};

// All the calls and their arg names are compiled into Entry Points for the runtime.
// #[no_mangle] macro ensures that the function name will be the same string in WASM.
#[no_mangle]
pub extern "C" fn call() {
    // The key shouldn't already exist in the named keys.
    let counter_local_key = storage::new_uref(0_i32);

    let mut vestor_named_keys: BTreeMap<String, Key> = BTreeMap::new();
    let key_name = String::from(CONTRACT_NAME);
    vestor_named_keys.insert(key_name, counter_local_key.into());

    let admin_key = storage::new_uref(runtime::get_caller());
    vestor_named_keys.insert(String::from(KEY_NAME_ADMIN), admin_key.into());

    let initialized_key = storage::new_uref(false);
    vestor_named_keys.insert(String::from(KEY_NAME_INITIALIZED), initialized_key.into());

    let (stored_contract_hash, contract_version) = storage::new_locked_contract(
        get_entry_points(),
        Some(vestor_named_keys),
        Some(String::from(VESTOR_PACKAGE_NAME)),
        Some(String::from(VESTOR_UREF_NAME)),
    );

    let version_uref = storage::new_uref(contract_version);

    runtime::put_key(CONTRACT_VERSION, version_uref.into());

    runtime::put_key(CONTRACT_HASH, stored_contract_hash.into());
}

#[no_mangle]
pub extern "C" fn init() {
    let contract_package_hash: Key = runtime::get_named_arg(ARG_NAME_ERC20_SELFCONTRACT_HASH);
    let hash = contract_package_hash.into_hash();
    let p_hash = ContractPackageHash::new(hash.unwrap());
    StakeContract::default().init(p_hash);
}

#[no_mangle]
pub extern "C" fn add_pool() {
    let staking_token_string: String = runtime::get_named_arg(ARG_NAME_STAKING_TOKEN);
    let staking_token_hash = ContractHash::from_formatted_str(staking_token_string.as_str())
        .expect("lock token hash string format is error");

    let reward_token_string: String = runtime::get_named_arg(ARG_NAME_REWARD_TOKEN);
    let reward_token_hash = ContractHash::from_formatted_str(reward_token_string.as_str())
        .expect("lock token hash string format is error");

    let start_time: u64 = runtime::get_named_arg(ARG_NAME_START_TIME);
    let end_time: u64 = runtime::get_named_arg(ARG_NAME_END_TIME);
    let precision: u64 = runtime::get_named_arg(ARG_NAME_PRECISION);
    let total_reward: U256 = runtime::get_named_arg(ARG_NAME_TOTAL_REWARD);

    StakeContract::default().add_pool(
        staking_token_hash,
        reward_token_hash,
        start_time,
        end_time,
        precision,
        total_reward,
    );
}

#[no_mangle]
pub extern "C" fn deposit() {
    let amount: U256 = runtime::get_named_arg(ARG_NAME_AMOUNT);
    let pool_id: u64 = runtime::get_named_arg(ARG_NAME_POOL_ID);

    StakeContract::default().deposit(amount, pool_id);
}

#[no_mangle]
pub extern "C" fn withdraw() {
    let amount: U256 = runtime::get_named_arg(ARG_NAME_AMOUNT);
    let pool_id: u64 = runtime::get_named_arg(ARG_NAME_POOL_ID);

    StakeContract::default().withdraw(amount, pool_id);
}

#[no_mangle]
pub extern "C" fn emergency_withdraw() {
    let pool_id: u64 = runtime::get_named_arg(ARG_NAME_POOL_ID);

    StakeContract::default().emergency_withdraw(pool_id);
}

#[no_mangle]
pub extern "C" fn stop_reward() {
    let pool_id: u64 = runtime::get_named_arg(ARG_NAME_POOL_ID);

    StakeContract::default().stop_reward(pool_id);
}

#[no_mangle]
pub extern "C" fn save_me() {
    let token_hash_str: String = runtime::get_named_arg(ARG_NAME_TOKEN_HASH);
    let token_hash = ContractHash::from_formatted_str(token_hash_str.as_str())
        .expect("lock token hash string format is error");

    let amount: U256 = runtime::get_named_arg(ARG_NAME_AMOUNT);

    StakeContract::default().save_me(token_hash, amount);
}

#[no_mangle]
pub extern "C" fn set_admin() {
    let new_admin_hash: AccountHash = runtime::get_named_arg(ARG_NAME_NEW_ADMIN);

    StakeContract::default().set_admin(new_admin_hash);
}

fn get_entry_points() -> EntryPoints {
    let mut entry_points = EntryPoints::new();

    entry_points.add_entry_point(EntryPoint::new(
        ENTRYPOINT_NAME_INIT,
        vec![Parameter::new(
            ARG_NAME_ERC20_SELFCONTRACT_HASH,
            String::cl_type(),
        )],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRYPOINT_NAME_ADD_POOL,
        vec![
            Parameter::new(ARG_NAME_STAKING_TOKEN, String::cl_type()),
            Parameter::new(ARG_NAME_REWARD_TOKEN, String::cl_type()),
            Parameter::new(ARG_NAME_START_TIME, u64::cl_type()),
            Parameter::new(ARG_NAME_END_TIME, u64::cl_type()),
            Parameter::new(ARG_NAME_PRECISION, u64::cl_type()),
            Parameter::new(ARG_NAME_TOTAL_REWARD, U256::cl_type()),
        ],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRYPOINT_NAME_DEPOSIT,
        vec![
            Parameter::new(ARG_NAME_AMOUNT, U256::cl_type()),
            Parameter::new(ARG_NAME_POOL_ID, u64::cl_type()),
        ],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRYPOINT_NAME_WITHDRAW,
        vec![
            Parameter::new(ARG_NAME_AMOUNT, U256::cl_type()),
            Parameter::new(ARG_NAME_POOL_ID, u64::cl_type()),
        ],
        CLType::U256,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRYPOINT_NAME_EMERGENCY_WITHDRAW,
        vec![Parameter::new(ARG_NAME_POOL_ID, u64::cl_type())],
        CLType::U64,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRYPOINT_NAME_STOP_REWARD,
        vec![Parameter::new(ARG_NAME_POOL_ID, u64::cl_type())],
        CLType::U256,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRYPOINT_NAME_SAVE_ME,
        vec![
            Parameter::new(ARG_NAME_TOKEN_HASH, String::cl_type()),
            Parameter::new(ARG_NAME_AMOUNT, U256::cl_type()),
        ],
        CLType::U256,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRYPOINT_NAME_SET_ADMIN,
        vec![Parameter::new(ARG_NAME_NEW_ADMIN, AccountHash::cl_type())],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points
}
