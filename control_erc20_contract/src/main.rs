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
    CLType, CLTyped, ContractHash, Key, Parameter, PublicKey, U256,
};

use contract::constants::{
    ARG_NAME_CLIFF_AMOUNT, ARG_NAME_ERC20_SELFCONTRACT_HASH, ARG_NAME_ERC20_TOKEN_HASH,
    ARG_NAME_NEW_ADMIN, ARG_NAME_NEW_OWNER, ARG_NAME_NEW_RELEASE_TIME, CONTRACT_HASH,
    CONTRACT_NAME, CONTRACT_VERSION, ENTRY_POINT_NAME_CLAIM, ENTRY_POINT_NAME_EXTEND_LOCK,
    ENTRY_POINT_NAME_INIT, ENTRY_POINT_NAME_LOCK, ENTRY_POINT_NAME_SET_ADMIN,
    ENTRY_POINT_NAME_SET_PAUSE_CONTRACT, ENTRY_POINT_NAME_TRANSFER_LOCK, KEY_NAME_INITIALIZED,
    KEY_NAME_PAUSED, VESTOR_PACKAGE_NAME, VESTOR_UREF_NAME,
};
use contract::{
    constants::{
        ARG_NAME_CLIFF_DURTIME, ARG_NAME_INDEX, ARG_NAME_LOCK_ID, ARG_NAME_NUM_UNLOCKS,
        ARG_NAME_TIME_BETWEEN_LOCKS, KEY_NAME_ADMIN,
    },
    VestContract,
};

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

    let paused_key = storage::new_uref(false);
    vestor_named_keys.insert(String::from(KEY_NAME_PAUSED), paused_key.into());

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
    let contract_package_hash: String = runtime::get_named_arg(ARG_NAME_ERC20_SELFCONTRACT_HASH);
    VestContract::default().init(contract_package_hash);
}

#[no_mangle]
pub extern "C" fn lock() {
    let str_hash_token: String = runtime::get_named_arg(ARG_NAME_ERC20_TOKEN_HASH);
    let hash_token = ContractHash::from_formatted_str(str_hash_token.as_str())
        .expect("lock token hash string format is error");

    let cliff_amount: U256 = runtime::get_named_arg(ARG_NAME_CLIFF_AMOUNT);
    let cliff_durtime: u64 = runtime::get_named_arg(ARG_NAME_CLIFF_DURTIME);
    let time_between_locks: u64 = runtime::get_named_arg(ARG_NAME_TIME_BETWEEN_LOCKS);
    let number_of_locks: u32 = runtime::get_named_arg(ARG_NAME_NUM_UNLOCKS);

    VestContract::default().lock(
        hash_token,
        cliff_amount,
        cliff_durtime,
        time_between_locks,
        number_of_locks,
    );
}

#[no_mangle]
pub extern "C" fn extend_lock() {
    let new_release_time: u64 = runtime::get_named_arg(ARG_NAME_NEW_RELEASE_TIME);
    let lock_id: u64 = runtime::get_named_arg(ARG_NAME_LOCK_ID);
    let index: u32 = runtime::get_named_arg(ARG_NAME_INDEX);

    VestContract::default().extend_lock(lock_id, index, new_release_time);
}

#[no_mangle]
pub extern "C" fn claim() {
    let lock_id: u64 = runtime::get_named_arg(ARG_NAME_LOCK_ID);

    VestContract::default().claim(lock_id);
}

#[no_mangle]
pub extern "C" fn transfer_lock() {
    let lock_id: u64 = runtime::get_named_arg(ARG_NAME_LOCK_ID);
    let str_account_new_owner: PublicKey = runtime::get_named_arg(ARG_NAME_NEW_OWNER);
    let x: AccountHash = str_account_new_owner.to_account_hash();

    VestContract::default().transfer_lock(lock_id, x);
}

#[no_mangle]
pub extern "C" fn set_admin() {
    let str_account_new_admin: PublicKey = runtime::get_named_arg(ARG_NAME_NEW_ADMIN);
    let new_admin_hash: AccountHash = str_account_new_admin.to_account_hash();

    VestContract::default().set_admin(new_admin_hash);
}

#[no_mangle]
pub extern "C" fn set_pause_contract() {
    VestContract::default().set_pause_contract();
}

fn get_entry_points() -> EntryPoints {
    let mut entry_points = EntryPoints::new();

    entry_points.add_entry_point(EntryPoint::new(
        ENTRY_POINT_NAME_INIT,
        vec![Parameter::new(
            ARG_NAME_ERC20_SELFCONTRACT_HASH,
            String::cl_type(),
        )],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRY_POINT_NAME_LOCK,
        vec![
            Parameter::new(ARG_NAME_ERC20_TOKEN_HASH, String::cl_type()),
            Parameter::new(ARG_NAME_CLIFF_DURTIME, u64::cl_type()),
            Parameter::new(ARG_NAME_CLIFF_AMOUNT, U256::cl_type()),
            Parameter::new(ARG_NAME_TIME_BETWEEN_LOCKS, u64::cl_type()),
            Parameter::new(ARG_NAME_NUM_UNLOCKS, u32::cl_type()),
        ],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRY_POINT_NAME_CLAIM,
        vec![Parameter::new(ARG_NAME_LOCK_ID, u64::cl_type())],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRY_POINT_NAME_TRANSFER_LOCK,
        vec![
            Parameter::new(ARG_NAME_LOCK_ID, u64::cl_type()),
            Parameter::new(ARG_NAME_NEW_OWNER, PublicKey::cl_type()),
        ],
        CLType::U256,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRY_POINT_NAME_EXTEND_LOCK,
        vec![
            Parameter::new(ARG_NAME_LOCK_ID, u64::cl_type()),
            Parameter::new(ARG_NAME_NEW_RELEASE_TIME, u64::cl_type()),
            Parameter::new(ARG_NAME_INDEX, u32::cl_type()),
        ],
        CLType::U64,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRY_POINT_NAME_SET_ADMIN,
        vec![Parameter::new(ARG_NAME_NEW_ADMIN, PublicKey::cl_type())],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points.add_entry_point(EntryPoint::new(
        ENTRY_POINT_NAME_SET_PAUSE_CONTRACT,
        vec![],
        CLType::I32,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    ));

    entry_points
}
