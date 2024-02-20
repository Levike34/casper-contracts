extern crate alloc;

use crate::pool;

use pool::{DepositScenario, StakePool, UserInfo, WithdrawScenario};

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

use casper_types::{account::AccountHash, ApiError, ContractHash, ContractPackageHash, U256};

use crate::{constants::KEY_NAME_INDEX, utils};
use crate::{constants::KEY_NAME_INITIALIZED, interact_token::interact_erc20};

use crate::constants::{
    KEY_NAME_ADMIN, KEY_NAME_DIC_STAKE_INFOS, KEY_NAME_SELF_CONTRACT_HASH, KEY_NAME_USER_INFOS,
};

#[derive(Default)]
pub struct StakeContract;

impl StakeContract {
    // init to create dictionary for lock info
    pub fn init(&self, conthash: ContractPackageHash) {
        let is_initialized: bool = utils::get_key(KEY_NAME_INITIALIZED);

        if is_initialized {
            revert(ApiError::PermissionDenied)
        }

        storage::new_dictionary(KEY_NAME_DIC_STAKE_INFOS).unwrap_or_revert();
        storage::new_dictionary(KEY_NAME_USER_INFOS).unwrap_or_revert();

        set_self_contract_hash(conthash);

        utils::set_key(KEY_NAME_INDEX, 0u64);
        utils::set_key(KEY_NAME_INITIALIZED, true);
    }

    // create a new staking pool.
    pub fn add_pool(
        &self,
        staking_token: ContractHash,
        reward_token: ContractHash,
        start_time: u64,
        end_time: u64,
        precision: u64,
        total_reward: U256,
    ) {
        let timestamp: u64 = runtime::get_blocktime().into();

        if start_time < timestamp || end_time < timestamp {
            revert(ApiError::InvalidArgument)
        }

        if total_reward.is_zero() {
            revert(ApiError::InvalidArgument)
        }

        if !(18..=36).contains(&precision) {
            revert(ApiError::InvalidArgument)
        }

        let seed_uref = *runtime::get_key(KEY_NAME_DIC_STAKE_INFOS)
            .unwrap_or_revert()
            .as_uref()
            .unwrap_or_revert();

        let current_index: u64 = utils::get_key(KEY_NAME_INDEX);

        let new_pool = StakePool {
            id: current_index,
            last_reward_timestamp: 0u64,
            staking_token,
            reward_token,
            start_time,
            end_time,
            precision,
            owner: runtime::get_caller(),
            acc_token_per_share: U256::zero(),
            total_reward,
            total_staked: U256::zero(),
        };

        let res = StakePool::pack(&new_pool);

        let dictionary_item_key = make_pool_key_id(current_index);

        storage::dictionary_put(seed_uref, &dictionary_item_key, res);

        // transfer tokens to this contract
        interact_erc20::default().transfer_from(
            reward_token,
            runtime::get_caller(),
            self_contract_hash(),
            total_reward,
        );

        // update the global counter for lock_id
        utils::set_key(KEY_NAME_INDEX, current_index + 1u64);
    }

    // deposit staking tokens to earn reward tokens.
    pub fn deposit(&self, amount: U256, pool_id: u64) {
        if amount.is_zero() {
            revert(ApiError::InvalidArgument)
        }

        let mut stake_pool = update_pool(pool_id);

        let timestamp: u64 = runtime::get_blocktime().into();

        if stake_pool.end_time < timestamp {
            revert(ApiError::InvalidArgument)
        }

        let mut user_info = get_user_info_for_pool(pool_id);

        let new_amount = amount + user_info.amount;

        // first deposit
        if user_info.amount.is_zero() {
            user_info.amount = new_amount;

            stake_pool.total_staked += amount;

            user_info.reward_debt = new_amount
                .checked_mul(stake_pool.acc_token_per_share)
                .unwrap()
                .checked_div(U256::from(10).pow(U256::from(stake_pool.precision)))
                .unwrap();

            // transfer tokens to this contract
            interact_erc20::default().transfer_from(
                stake_pool.staking_token,
                runtime::get_caller(),
                self_contract_hash(),
                amount,
            );
            update_storage(&stake_pool, user_info, pool_id)
        } else {
            let acc_token_per_share = stake_pool.acc_token_per_share;

            let precision = U256::from(10).pow(U256::from(stake_pool.precision));

            let mut pending = user_info.amount * acc_token_per_share;
            pending /= precision;
            pending -= user_info.reward_debt;

            // update amounts
            user_info.amount = new_amount;
            stake_pool.total_staked += amount;
            user_info.reward_debt = (new_amount * acc_token_per_share) / precision;

            let scenario = DepositScenario::get_scenario(
                pending,
                amount,
                stake_pool.reward_token,
                stake_pool.staking_token,
            );

            match scenario {
                DepositScenario::PendingZero => interact_erc20::default().transfer_from(
                    stake_pool.staking_token,
                    runtime::get_caller(),
                    self_contract_hash(),
                    amount,
                ),
                DepositScenario::EqualTokenPendingGreater => interact_erc20::default().transfer(
                    stake_pool.staking_token,
                    runtime::get_caller(),
                    pending - amount,
                ),
                DepositScenario::EqualTokenPendingLesser => interact_erc20::default()
                    .transfer_from(
                        stake_pool.staking_token,
                        runtime::get_caller(),
                        self_contract_hash(),
                        amount - pending,
                    ),
                DepositScenario::ClaimRewardAndDeposit => {
                    interact_erc20::default().transfer(
                        stake_pool.reward_token,
                        runtime::get_caller(),
                        pending,
                    );
                    interact_erc20::default().transfer_from(
                        stake_pool.staking_token,
                        runtime::get_caller(),
                        self_contract_hash(),
                        amount,
                    )
                }
            };

            // update information at end in case
            // token transfer reverts
            update_storage(&stake_pool, user_info, pool_id)
        }
    }

    // withdraw stake and claim reward token.
    pub fn withdraw(&self, amount: U256, pool_id: u64) {
        if amount.is_zero() {
            revert(ApiError::InvalidArgument)
        }
        let mut user_info = get_user_info_for_pool(pool_id);
        let mut stake_pool = update_pool(pool_id);

        if user_info.amount.is_zero() {
            revert(ApiError::InvalidArgument)
        }

        let new_amount = user_info.amount - amount;

        let acc_token_per_share = stake_pool.acc_token_per_share;

        let precision = U256::from(10).pow(U256::from(stake_pool.precision));

        let mut pending = user_info.amount * acc_token_per_share;
        pending /= precision;
        pending -= user_info.reward_debt;

        user_info.amount = new_amount;
        stake_pool.total_staked -= amount;
        user_info.reward_debt = (new_amount * acc_token_per_share) / precision;

        let scenario = WithdrawScenario::get_scenario(
            pending,
            stake_pool.reward_token,
            stake_pool.staking_token,
        );

        match scenario {
            WithdrawScenario::PendingZero => {
                interact_erc20::default().transfer(
                    stake_pool.staking_token,
                    runtime::get_caller(),
                    amount,
                );
            }
            WithdrawScenario::EqualTokens => interact_erc20::default().transfer(
                stake_pool.staking_token,
                runtime::get_caller(),
                pending + amount,
            ),
            WithdrawScenario::DifferentTokens => {
                interact_erc20::default().transfer(
                    stake_pool.reward_token,
                    runtime::get_caller(),
                    pending,
                );
                interact_erc20::default().transfer(
                    stake_pool.staking_token,
                    runtime::get_caller(),
                    amount,
                )
            }
        };

        // update information at end in case
        // token transfer reverts
        update_storage(&stake_pool, user_info, pool_id)
    }

    // Ends a stake pool early and returns
    // tokens to pool owner
    pub fn stop_reward(&self, pool_id: u64) {
        let mut pool = update_pool(pool_id);

        if pool.owner != runtime::get_caller() {
            revert(ApiError::InvalidPurse)
        }

        let now = runtime::get_blocktime().into();

        let old_end_time = pool.end_time;

        if old_end_time <= now {
            revert(ApiError::InvalidArgument)
        }

        pool.end_time = now;

        let duration = U256::from(old_end_time - pool.start_time);

        let amount = U256::from(old_end_time - get_max(now, pool.start_time)) * pool.total_reward;

        interact_erc20::default().transfer(
            pool.staking_token,
            runtime::get_caller(),
            amount / duration,
        );

        let updated_pool = StakePool::pack(&pool);

        let seed_uref = *runtime::get_key(KEY_NAME_DIC_STAKE_INFOS)
            .unwrap_or_revert_with(ApiError::User(1))
            .as_uref()
            .unwrap_or_revert_with(ApiError::User(2));

        let dictionary_item_key = make_pool_key_id(pool_id);

        storage::dictionary_put::<Vec<u8>>(seed_uref, &dictionary_item_key, updated_pool);
    }

    // emergency function for saving funds if
    // something goes wrong.
    // !! EMERGENCY USE ONLY !!
    pub fn save_me(&self, token_hash: ContractHash, amount: U256) {
        let admin = utils::get_key(KEY_NAME_ADMIN);

        if runtime::get_caller() != admin {
            revert(ApiError::InvalidPurse)
        }

        interact_erc20::default().transfer(token_hash, runtime::get_caller(), amount)
    }
    // withdraw without caring about rewards.
    // !! EMERGENCY USE ONLY !!
    pub fn emergency_withdraw(&self, pool_id: u64) {
        let mut stake_pool = get_pool(pool_id);
        let mut user_info = get_user_info_for_pool(pool_id);
        let amount = user_info.amount;

        if amount.is_zero() {
            revert(ApiError::InvalidArgument)
        }

        user_info.amount = U256::zero();
        stake_pool.total_staked -= amount;
        user_info.reward_debt = U256::zero();

        interact_erc20::default().transfer(stake_pool.staking_token, runtime::get_caller(), amount);

        update_storage(&stake_pool, user_info, pool_id);
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
}

// returns the pool with updated values
fn update_pool(pool_id: u64) -> StakePool {
    let now = runtime::get_blocktime().into();

    let mut pool = get_pool(pool_id);

    let last_reward_timestamp = pool.last_reward_timestamp;
    if now <= last_reward_timestamp {
        pool
    } else {
        let lp_supply = pool.total_staked;

        if lp_supply.is_zero() || pool.start_time > now {
            pool.last_reward_timestamp = now;
            pool
        } else {
            let end = pool.end_time;

            if last_reward_timestamp > end {
                return pool;
            }

            let start = pool.start_time;
            let precision = U256::from(10).pow(U256::from(pool.precision));

            let reward_unlocked =
                U256::from(get_min(now, end) - get_max(start, last_reward_timestamp));

            let mut rewards = reward_unlocked * pool.total_reward;
            rewards /= U256::from(end - start);
            rewards *= precision;

            pool.acc_token_per_share += rewards / pool.total_staked;
            pool.last_reward_timestamp = now;
            pool
        }
    }
}

// ensure id is less than current index
fn is_id_valid(input: u64) {
    let current_index: u64 = utils::get_key(KEY_NAME_INDEX);

    if current_index < input {
        revert(ApiError::InvalidArgument)
    }
}

fn get_max(a: u64, b: u64) -> u64 {
    if a > b {
        a
    } else {
        b
    }
}

fn get_min(a: u64, b: u64) -> u64 {
    if a > b {
        b
    } else {
        a
    }
}

// retrieve lock and validate caller and lock info
fn get_pool(pool_id: u64) -> StakePool {
    is_id_valid(pool_id);

    let seed_uref = *runtime::get_key(KEY_NAME_DIC_STAKE_INFOS)
        .unwrap_or_revert_with(ApiError::User(5))
        .as_uref()
        .unwrap_or_revert_with(ApiError::User(6));

    let dictionary_item_key = make_pool_key_id(pool_id);

    let stake_pool_bytes = storage::dictionary_get::<Vec<u8>>(seed_uref, &dictionary_item_key)
        .unwrap_or_revert_with(ApiError::MissingKey)
        .unwrap_or_revert_with(ApiError::User(7));

    StakePool::unpack(stake_pool_bytes)
}

// retrieve stake pool and validate caller and lock info
fn update_storage(stake_pool: &StakePool, user_info: UserInfo, pool_id: u64) {
    is_id_valid(pool_id);

    let new_user_info = UserInfo::pack(&user_info);
    let updated_pool = StakePool::pack(stake_pool);

    let seed_uref = *runtime::get_key(KEY_NAME_DIC_STAKE_INFOS)
        .unwrap_or_revert_with(ApiError::User(1))
        .as_uref()
        .unwrap_or_revert_with(ApiError::User(2));

    let user_seed_uref = *runtime::get_key(KEY_NAME_USER_INFOS)
        .unwrap_or_revert_with(ApiError::User(3))
        .as_uref()
        .unwrap_or_revert_with(ApiError::User(4));

    let dictionary_item_key = make_pool_key_id(pool_id);
    let user_info_key = make_user_key_by_id(pool_id);

    storage::dictionary_put::<Vec<u8>>(seed_uref, &dictionary_item_key, updated_pool);
    storage::dictionary_put::<Vec<u8>>(user_seed_uref, &user_info_key, new_user_info);
}

// retrieve user info
fn get_user_info_for_pool(pool_id: u64) -> UserInfo {
    is_id_valid(pool_id);

    let seed_uref = *runtime::get_key(KEY_NAME_USER_INFOS)
        .unwrap_or_revert()
        .as_uref()
        .unwrap_or_revert();

    let dictionary_item_key = make_user_key_by_id(pool_id);

    let user_info_bytes_option =
        storage::dictionary_get::<Vec<u8>>(seed_uref, &dictionary_item_key);

    match user_info_bytes_option.unwrap() {
        Some(user) => UserInfo::unpack(user),
        None => UserInfo::default(),
    }
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
fn set_self_contract_hash(conthash: ContractPackageHash) {
    utils::set_key(KEY_NAME_SELF_CONTRACT_HASH, conthash);
}

/*
for organizing pools by index
*/
fn make_pool_key_id(pool_id: u64) -> String {
    let c_hash = self_contract_hash().to_string();
    let (pre_key, _) = c_hash.split_at(15);
    let append = pool_id.to_string();

    String::from(pre_key) + &append
}

/*
for organizing pools by index
*/
fn make_user_key_by_id(pool_id: u64) -> String {
    let account_hash = runtime::get_caller().to_string();
    let (pre_key, _) = account_hash.split_at(15);
    let append = pool_id.to_string();

    String::from(pre_key) + &append
}
