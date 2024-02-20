//#![no_std]
#![no_main]
#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(non_camel_case_types)]
#![allow(unused_attributes)]

// #[cfg(not(target_arch = "wasm32"))]
// compile_error!("target arch should be wasm32: compile with '--target wasm32-unknown-unknown'");

extern crate alloc;
use core::str::FromStr;

use casper_contract::{
    contract_api::{runtime, storage},
    unwrap_or_revert::UnwrapOrRevert,
};
use casper_types::{
    account::AccountHash, bytesrepr::ToBytes, runtime_args, ContractHash, ContractPackageHash, Key,
    RuntimeArgs, U256,
};

use alloc::{
    borrow::ToOwned,
    collections::BTreeMap, //BTreeSet},
    string::String,
};

use crate::utils;
use crate::{Address, Error};

use crate::constants::{
    ARG_NAME_AMOUNT, ARG_NAME_OWNER, ARG_NAME_RECIPIENT, ARG_NAME_SPENDER,
    ENTRY_POINT_NAME_BALANCE_OF, ENTRY_POINT_NAME_TRANSFER, ENTRY_POINT_NAME_TRANSFER_FROM,
    KEY_NAME_TOKEN_HASH,
};

/*
implement ERC20 functionality for vesting
*/
#[derive(Default)]
pub struct interact_erc20;

impl interact_erc20 {
    pub fn get_token_hash(&self) -> ContractHash {
        utils::get_key(KEY_NAME_TOKEN_HASH)
    }

    pub fn transfer_from(
        &mut self,
        hash_token: ContractHash,
        owner: AccountHash,
        spender: ContractPackageHash,
        amount: U256,
    ) {
        runtime::call_contract(
            hash_token, //contracthash
            ENTRY_POINT_NAME_TRANSFER_FROM,
            runtime_args! {
                ARG_NAME_OWNER => Address::from(owner),         //owner : AccountHash
                ARG_NAME_RECIPIENT => Address::from(spender),   //spender: AccountHash
                ARG_NAME_AMOUNT => amount
            },
        )
    }

    pub fn transfer(&mut self, hash_token: ContractHash, recipient: AccountHash, amount: U256) {
        runtime::call_contract(
            hash_token, //self.get_token_hash(),
            ENTRY_POINT_NAME_TRANSFER,
            runtime_args! {
                ARG_NAME_RECIPIENT => Address::from(recipient),
                ARG_NAME_AMOUNT => amount
            },
        )
    }
}
