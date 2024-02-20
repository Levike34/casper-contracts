extern crate alloc;

use core::convert::TryInto;

use alloc::vec::Vec;

use casper_contract::unwrap_or_revert::UnwrapOrRevert;

use casper_types::{account::AccountHash, bytesrepr::ToBytes, ContractHash, U256};

#[derive(Clone, Copy)]
pub struct UserInfo {
    // 32
    pub amount: U256,
    // 32 : 64
    pub reward_debt: U256,
}

impl UserInfo {
    pub fn pack(&self) -> Vec<u8> {
        let mut amount_bytes = [0_u8; 64];
        self.amount.to_little_endian(&mut amount_bytes[..32]);

        let mut reward_debt_bytes = [0_u8; 64];
        self.reward_debt
            .to_little_endian(&mut reward_debt_bytes[..32]);

        let mut res: Vec<u8> = Vec::new();

        for i in amount_bytes.iter().take(32) {
            res.push(*i)
        }

        for i in 32..64 {
            res.push(reward_debt_bytes[i - 32])
        }

        res
    }

    pub fn unpack(src: Vec<u8>) -> Self {
        let amount = U256::from_little_endian(&src[0..32]);
        let reward_debt = U256::from_little_endian(&src[32..64]);

        Self {
            amount,
            reward_debt,
        }
    }

    pub fn default() -> Self {
        Self {
            amount: U256::zero(),
            reward_debt: U256::zero(),
        }
    }
}

pub struct StakePool {
    // 8 : 8
    pub id: u64,
    // 8 : 16,
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

impl StakePool {
    // returns all lock info as a byte array
    pub fn pack(&self) -> Vec<u8> {
        let id_bytes = self.id.into_bytes().unwrap_or_revert();
        let last_reward_timestamp = self.last_reward_timestamp.into_bytes().unwrap_or_revert();
        let staking_token_bytes = self.staking_token.as_bytes();
        let reward_token_bytes = self.reward_token.as_bytes();
        let start_time_bytes = self.start_time.into_bytes().unwrap_or_revert();
        let end_time_bytes = self.end_time.into_bytes().unwrap_or_revert();
        let precision_bytes = self.precision.into_bytes().unwrap_or_revert();
        let owner_bytes = self.owner.as_bytes();

        let mut total_reward_bytes = [0_u8; 64];
        self.total_reward
            .to_little_endian(&mut total_reward_bytes[..32]);

        let mut acc_token_per_share_bytes = [0_u8; 64];
        self.acc_token_per_share
            .to_little_endian(&mut acc_token_per_share_bytes[..32]);

        let mut total_staked_bytes = [0_u8; 64];
        self.total_staked
            .to_little_endian(&mut total_staked_bytes[..32]);

        let mut res: Vec<u8> = Vec::new();

        for i in id_bytes {
            res.push(i)
        }

        for i in 8..16 {
            res.push(last_reward_timestamp[i - 8])
        }

        for i in 16..48 {
            res.push(staking_token_bytes[i - 16])
        }

        for i in 48..80 {
            res.push(reward_token_bytes[i - 48])
        }

        for i in 80..88 {
            res.push(start_time_bytes[i - 80])
        }

        for i in 88..96 {
            res.push(end_time_bytes[i - 88])
        }

        for i in 96..104 {
            res.push(precision_bytes[i - 96])
        }

        for i in 104..136 {
            res.push(owner_bytes[i - 104])
        }

        for i in 136..168 {
            res.push(total_reward_bytes[i - 136])
        }

        for i in 168..200 {
            res.push(acc_token_per_share_bytes[i - 168])
        }

        for i in 200..232 {
            res.push(total_staked_bytes[i - 200])
        }
        res
    }

    pub fn unpack(src: Vec<u8>) -> Self {
        let id = u64::from_le_bytes(src[0..8].try_into().unwrap());
        let last_reward_timestamp = u64::from_le_bytes(src[8..16].try_into().unwrap());
        let staking_token: ContractHash = src[16..48].try_into().unwrap();
        let reward_token: ContractHash = src[48..80].try_into().unwrap();

        let start_time = u64::from_le_bytes(src[80..88].try_into().unwrap());
        let end_time = u64::from_le_bytes(src[88..96].try_into().unwrap());
        let precision = u64::from_le_bytes(src[96..104].try_into().unwrap());

        let owner: AccountHash = src[104..136].try_into().unwrap();
        let total_reward = U256::from_little_endian(&src[136..168]);
        let acc_token_per_share = U256::from_little_endian(&src[168..200]);
        let total_staked = U256::from_little_endian(&src[200..232]);

        Self {
            id,
            last_reward_timestamp,
            staking_token,
            reward_token,
            start_time,
            end_time,
            precision,
            owner,
            total_reward,
            acc_token_per_share,
            total_staked,
        }
    }

    // gets the owner account from StakePool
    // used in caller_is_recipient() function
    pub fn unpack_owner(src: &[u8]) -> [u8; 32] {
        let owner: [u8; 32] = src[104..136].try_into().unwrap();
        owner
    }
}

pub enum DepositScenario {
    PendingZero,
    EqualTokenPendingGreater,
    EqualTokenPendingLesser,
    ClaimRewardAndDeposit,
}

impl DepositScenario {
    pub fn get_scenario(
        pending: U256,
        amount: U256,
        reward_token: ContractHash,
        staking_token: ContractHash,
    ) -> Self {
        if pending.is_zero() {
            return Self::PendingZero;
        }

        let pending_gt_amount: bool = pending > amount;

        let reward_token_eq_staking_token: bool = reward_token == staking_token;

        if pending_gt_amount && reward_token_eq_staking_token {
            return Self::EqualTokenPendingGreater;
        }

        if !pending_gt_amount && reward_token_eq_staking_token {
            return Self::EqualTokenPendingLesser;
        }

        Self::ClaimRewardAndDeposit
    }
}

pub enum WithdrawScenario {
    PendingZero,
    EqualTokens,
    DifferentTokens,
}

impl WithdrawScenario {
    pub fn get_scenario(
        pending: U256,
        reward_token: ContractHash,
        staking_token: ContractHash,
    ) -> Self {
        if pending.is_zero() {
            return Self::PendingZero;
        }

        let reward_token_eq_staking_token: bool = reward_token == staking_token;

        if reward_token_eq_staking_token {
            return Self::EqualTokens;
        }

        Self::DifferentTokens
    }
}
