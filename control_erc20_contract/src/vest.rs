extern crate alloc;

use core::convert::TryInto;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use casper_contract::{
    contract_api::{
        runtime::{self, revert},
        storage,
    },
    unwrap_or_revert::UnwrapOrRevert,
};

use casper_types::{
    account::AccountHash, bytesrepr::ToBytes, ApiError, ContractHash, ContractPackageHash, URef,
    U256,
};

use crate::{constants::KEY_NAME_INDEX, utils};
use crate::{constants::KEY_NAME_INITIALIZED, interact_token::interact_erc20};

use crate::constants::{
    KEY_NAME_ADMIN, KEY_NAME_DIC_LOCK_INFOS, KEY_NAME_PAUSED, KEY_NAME_SELF_CONTRACT_HASH,
};

#[derive(Default)]
pub struct VestContract;

#[derive(Clone, Copy)]
pub struct LockSchedule {
    // 8
    release: u64,
    // 32
    amount: U256,
}

pub struct VestInfo {
    // 8 : 8
    id: u64,
    // 8 : 16
    lock_time: u64,
    // 32 : 48
    recipient: AccountHash,
    // 32 : 80
    token_hash: ContractHash,
    // 40 : (80 + 40(num))
    schedules: Vec<LockSchedule>,
}

impl VestInfo {
    // returns all lock info as a byte array
    fn pack(&self) -> Vec<u8> {
        let id_bytes = self.id.into_bytes().unwrap_or_revert();
        let lock_time_bytes = self.lock_time.into_bytes().unwrap_or_revert();
        let recipient_bytes = self.recipient.as_bytes();
        let token_hash_bytes = self.token_hash.value();

        let mut res: Vec<u8> = Vec::new();

        for i in id_bytes {
            res.push(i);
        }

        for i in 8..16 {
            res.push(lock_time_bytes[i - 8])
        }

        for i in 16..48 {
            res.push(recipient_bytes[i - 16])
        }

        for i in 48..80 {
            res.push(token_hash_bytes[i - 48])
        }

        let mut counter = 0;

        while counter < self.schedules.len() {
            let slice_to_pack = &self.schedules[counter];
            let schedule_bytes = Self::pack_schedule(slice_to_pack);
            for i in schedule_bytes {
                res.push(i);
            }
            counter += 1
        }

        res
    }

    fn pack_schedule(src: &LockSchedule) -> [u8; 40] {
        let mut res = [0u8; 40];

        let mut little_endian_bytes = [0_u8; 64];

        src.amount.to_little_endian(&mut little_endian_bytes[..32]);

        let release = src.release.to_le_bytes();

        res[..8].copy_from_slice(&release[..8]);

        res[8..40].copy_from_slice(&little_endian_bytes[..32]);

        res
    }

    fn unpack(src: Vec<u8>) -> Self {
        let id = u64::from_le_bytes(src[0..8].try_into().unwrap());
        let lock_time = u64::from_le_bytes(src[8..16].try_into().unwrap());
        let recipient: AccountHash = src[16..48].try_into().unwrap();
        let token_hash: ContractHash = src[48..80].try_into().unwrap();

        let (_, slice_schedules) = src.split_at(80);

        let schedules = Self::unpack_all_schedules(slice_schedules.to_vec());

        Self {
            id,
            lock_time,
            recipient,
            token_hash,
            schedules,
        }
    }

    fn unpack_schedule(src: &[u8]) -> LockSchedule {
        let release = u64::from_le_bytes(src[0..8].try_into().unwrap());
        let amount = U256::from_little_endian(&src[8..40]);

        LockSchedule { release, amount }
    }

    fn unpack_all_schedules(src: Vec<u8>) -> Vec<LockSchedule> {
        if src.len() % 40 != 0 {
            revert(ApiError::Formatting)
        }

        let mut schedules: Vec<LockSchedule> = Vec::new();

        let mut counter = 0;
        let total_schedules = src.len().checked_div(40).unwrap_or_revert();

        while counter < total_schedules {
            let offset = counter.checked_mul(40).unwrap_or_revert();
            let end_idx = offset.checked_add(40).unwrap_or_revert();
            let s: LockSchedule = Self::unpack_schedule(&src[offset..end_idx]);
            schedules.push(s);
            counter += 1
        }
        schedules
    }

    // gets the recipient account from LockInfo
    // used in caller_is_recipient() function
    fn unpack_recipient(src: &[u8]) -> [u8; 32] {
        let recipient: [u8; 32] = src[16..48].try_into().unwrap();
        recipient
    }

    fn clear_entry() -> Vec<u8> {
        let clear_entry = [0_u8; 0];
        clear_entry.to_vec()
    }
}

impl VestContract {
    // init to create dictionary for lock info
    pub fn init(&self, conthash: String) {
        let is_initialized: bool = utils::get_key(KEY_NAME_INITIALIZED);

        if is_initialized {
            revert(ApiError::PermissionDenied)
        }

        storage::new_dictionary(KEY_NAME_DIC_LOCK_INFOS).unwrap_or_revert();

        set_self_contract_hash(conthash);

        utils::set_key(KEY_NAME_INDEX, 0u64);
        utils::set_key(KEY_NAME_INITIALIZED, true);
    }

    // @tokh - token to lock
    // @cliff_amount - amount to lock
    // @cliff_durtime - time from now until first unlock
    // @time_between_locks - seconds between each unlock
    // @number_of_locks - number of locks
    pub fn lock(
        &self,
        tokh: ContractHash,
        cliff_amount: U256,
        cliff_durtime: u64,
        time_between_locks: u64,
        number_of_locks: u32,
    ) {
        if is_paused() {
            revert(ApiError::User(14))
        }
        let seed_uref = *runtime::get_key(KEY_NAME_DIC_LOCK_INFOS)
            .unwrap_or_revert()
            .as_uref()
            .unwrap_or_revert();

        let mut schedules: Vec<LockSchedule> = Vec::new();

        let timestamp: u64 = runtime::get_blocktime().into();

        let first_unlock: u64 = timestamp.checked_add(cliff_durtime).unwrap_or_revert();

        if number_of_locks == 0 {
            revert(ApiError::InvalidArgument)
        }

        while schedules.len() < number_of_locks as usize {
            let release = first_unlock
                + (time_between_locks
                    .checked_mul(schedules.len() as u64)
                    .unwrap_or_revert());
            let amount = cliff_amount / U256::from(number_of_locks);
            let s = LockSchedule { release, amount };
            schedules.push(s)
        }

        // lock_id
        let current_index: u64 = utils::get_key(KEY_NAME_INDEX);

        let dictionary_item_key = make_lock_key_id(current_index);

        let info: VestInfo = VestInfo {
            id: current_index,
            lock_time: timestamp,
            recipient: runtime::get_caller(),
            token_hash: tokh,
            schedules,
        };

        // pack lock information into u8 array
        let res = VestInfo::pack(&info);

        // transfer tokens to this contract
        let tx = interact_erc20::default().transfer_from(
            tokh,
            runtime::get_caller(),
            self_contract_hash(),
            cliff_amount,
        );

        if Some(tx).is_none() {
            revert(ApiError::None)
        }

        // This identifies an item within the dictionary
        // and either creates or updates the associated value.
        update_storage(
            seed_uref,
            dictionary_item_key,
            res,
            current_index,
            runtime::get_caller().to_string(),
        );

        // update the global counter for lock_id
        utils::set_key(KEY_NAME_INDEX, current_index + 1u64);
    }

    // @lock_id = the lock to change
    // @index = index of the lock to update
    // @new_release = new release time
    pub fn extend_lock(&mut self, lock_id: u64, index: u32, new_release: u64) {
        if is_paused() {
            revert(ApiError::User(14))
        }
        let (mut lock, seed_uref, dictionary_item_key) = get_lock(lock_id);

        // can't update non-existant schedule
        if index as usize >= lock.schedules.len() {
            revert(ApiError::InvalidArgument)
        }

        if lock.schedules[index as usize].release > new_release {
            revert(ApiError::None)
        }

        lock.schedules[index as usize].release = new_release;

        // pack updated lock information into u8 array
        let res = VestInfo::pack(&lock);

        // update key
        storage::dictionary_put(seed_uref, &dictionary_item_key, res)
    }

    // @lock_id = the lock to change
    // @new_owner = who to transfer to
    pub fn transfer_lock(&mut self, lock_id: u64, new_owner: AccountHash) {
        if is_paused() {
            revert(ApiError::User(14))
        }
        let (mut lock, seed_uref, dictionary_item_key) = get_lock(lock_id);

        lock.recipient = new_owner;

        // pack lock information into u8 array
        let res = VestInfo::pack(&lock);

        update_storage_transfer_lock(
            seed_uref,
            dictionary_item_key,
            lock.id,
            runtime::get_caller().to_string(),
            new_owner.to_string(),
            res,
        )
    }

    // unlock tokens
    // @lock_id - the lock to change
    pub fn claim(&self, lock_id: u64) {
        if is_paused() {
            revert(ApiError::User(14))
        }
        let (mut lock, seed_uref, dictionary_item_key) = get_lock(lock_id);

        let timestamp: u64 = runtime::get_blocktime().into();

        let mut amount_to_transfer = U256::zero();

        for s in lock.schedules.iter_mut() {
            if timestamp >= s.release {
                amount_to_transfer += s.amount;
                s.amount = U256::zero()
            }
        }

        if amount_to_transfer.is_zero() {
            revert(ApiError::User(9))
        }

        // transfer tokens from this contract
        let tx = interact_erc20::default().transfer(
            lock.token_hash,
            runtime::get_caller(),
            amount_to_transfer,
        );

        if Some(tx).is_none() {
            revert(ApiError::None)
        }

        let mut empty_locks: usize = 0;
        let total_locks = lock.schedules.len();

        for s in lock.schedules.iter() {
            if s.amount.is_zero() {
                empty_locks += 1
            }
        }

        let all_claimed: bool = total_locks == empty_locks;

        match all_claimed {
            false => {
                // pack lock information into u8 array
                let res = VestInfo::pack(&lock);
                // update key
                storage::dictionary_put(seed_uref, &dictionary_item_key, res)
            }
            _ => {
                // clear the entry if it is finished
                storage::dictionary_put(seed_uref, &dictionary_item_key, VestInfo::clear_entry());
                let mut id_arr: Vec<u64> =
                    utils::get_key(runtime::get_caller().to_string().as_str());
                let idx = id_arr.iter().position(|x| *x == lock_id).unwrap();
                id_arr.swap_remove(idx);
                utils::set_key(runtime::get_caller().to_string().as_str(), id_arr);
            }
        }
    }

    // sets new admin
    // @account - account hash of the new admin
    // ADMIN ONLY
    pub fn set_admin(&self, account: AccountHash) {
        let admin = utils::get_key(KEY_NAME_ADMIN);

        if runtime::get_caller() != admin {
            revert(ApiError::InvalidPurse)
        }

        utils::set_key(KEY_NAME_ADMIN, account)
    }

    // pause or unpause contract
    // ADMIN ONLY
    pub fn set_pause_contract(&self) {
        let admin = utils::get_key(KEY_NAME_ADMIN);

        if runtime::get_caller() != admin {
            revert(ApiError::InvalidPurse)
        }

        let current_state: bool = utils::get_key(KEY_NAME_PAUSED);

        utils::set_key(KEY_NAME_PAUSED, !current_state);
    }
}

// compares bytes from 16..48 (32) to see if it matches the caller
fn caller_is_recipient(src: &[u8]) {
    let caller_bytes: [u8; 32] = runtime::get_caller().as_bytes().try_into().unwrap();
    let recipient = VestInfo::unpack_recipient(src);

    if recipient != caller_bytes {
        runtime::revert(ApiError::InvalidPurse)
    }
}

// ensure id is less than current index
fn is_id_valid(input: u64) {
    let current_index: u64 = utils::get_key(KEY_NAME_INDEX);

    if current_index < input {
        revert(ApiError::InvalidArgument)
    }
}

// ensure entry exists
fn is_valid_entry(src: &[u8]) {
    if src.is_empty() {
        revert(ApiError::MissingKey)
    }
}

// retrieve lock and validate caller and lock info
fn get_lock(lock_id: u64) -> (VestInfo, URef, String) {
    is_id_valid(lock_id);

    let seed_uref = *runtime::get_key(KEY_NAME_DIC_LOCK_INFOS)
        .unwrap_or_revert()
        .as_uref()
        .unwrap_or_revert();

    let dictionary_item_key = make_lock_key_id(lock_id);

    let lock_bytes = storage::dictionary_get::<Vec<u8>>(seed_uref, &dictionary_item_key)
        .unwrap_or_revert_with(ApiError::MissingKey)
        .unwrap_or_revert();

    // check if info exist (hasn't been cleared)
    is_valid_entry(&lock_bytes);

    // check that the caller is the owner of the lock
    caller_is_recipient(&lock_bytes);

    let lock = VestInfo::unpack(lock_bytes);
    (lock, seed_uref, dictionary_item_key)
}

fn update_storage(
    seed_uref: URef,
    dictionary_item_key: String,
    res: Vec<u8>,
    index: u64,
    user_key: String,
) {
    storage::dictionary_put(seed_uref, &dictionary_item_key, res);
    let exists = runtime::has_key(&user_key);
    match exists {
        false => {
            let mut id_arr: Vec<u64> = Vec::new();
            id_arr.push(index);
            utils::set_key(&user_key, id_arr)
        }
        _ => {
            let mut id_arr: Vec<u64> = utils::get_key(&user_key);
            id_arr.push(index);
            utils::set_key(&user_key, id_arr);
        }
    }
}

fn update_storage_transfer_lock(
    seed_uref: URef,
    dictionary_item_key: String,
    index: u64,
    old_owner_key: String,
    new_owner_key: String,
    new_lock: Vec<u8>,
) {
    // get ids for old user
    let mut id_arr: Vec<u64> = utils::get_key(&old_owner_key);
    // remove it
    let idx = id_arr.iter().position(|x| *x == index).unwrap();
    id_arr.swap_remove(idx);
    // update storage
    utils::set_key(&old_owner_key, id_arr);
    // NEW USER
    storage::dictionary_put(seed_uref, &dictionary_item_key, new_lock);
    let exists = runtime::has_key(&new_owner_key);
    match exists {
        false => {
            let mut id_arr: Vec<u64> = Vec::new();
            id_arr.push(index);
            utils::set_key(&new_owner_key, id_arr)
        }
        _ => {
            let mut id_arr_2: Vec<u64> = utils::get_key(&new_owner_key);
            // add it
            id_arr_2.push(index);
            // update storage
            utils::set_key(&new_owner_key, id_arr_2);
        }
    }
}

fn is_paused() -> bool {
    utils::get_key(KEY_NAME_PAUSED)
}

/*
get contract hash
*/
fn self_contract_hash() -> ContractPackageHash {
    utils::get_key(KEY_NAME_SELF_CONTRACT_HASH)
}

/*
set contract hash - used in token transfer on claim() and lock( ).
*/
fn set_self_contract_hash(conthash: String) {
    let self_acc_hash = ContractPackageHash::from_formatted_str(conthash.as_str())
        .expect("self contract string format is error");
    utils::set_key(KEY_NAME_SELF_CONTRACT_HASH, self_acc_hash);
}

/*
for organizing data for locks by index
*/

fn make_lock_key_id(lock_id: u64) -> String {
    let c_hash = self_contract_hash().to_string();
    let (pre_key, _) = c_hash.split_at(15);
    let append = lock_id.to_string();

    String::from(pre_key) + &append
}
