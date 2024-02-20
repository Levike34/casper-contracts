#[cfg(test)]
mod tests {
    use std::{convert::TryInto, path::PathBuf};

    use casper_engine_test_support::{
        DeployItemBuilder, ExecuteRequestBuilder, InMemoryWasmTestBuilder, ARG_AMOUNT,
        DEFAULT_ACCOUNT_ADDR, DEFAULT_ACCOUNT_INITIAL_BALANCE, DEFAULT_GENESIS_CONFIG,
        DEFAULT_GENESIS_CONFIG_HASH, DEFAULT_PAYMENT, DEFAULT_RUN_GENESIS_REQUEST,
        MINIMUM_ACCOUNT_CREATION_BALANCE,
    };
    use casper_execution_engine::core::engine_state::{
        run_genesis_request::RunGenesisRequest, GenesisAccount,
    };
    use casper_types::{
        account::{Account, AccountHash},
        bytesrepr,
        bytesrepr::{Error, FromBytes, ToBytes},
        contracts::NamedKeys,
        runtime_args,
        system::mint,
        AsymmetricType, CLType, CLTyped, ContractHash, ContractPackageHash, HashAddr, Key, Motes,
        PublicKey, RuntimeArgs, SecretKey, StoredValue, URef, U256, U512,
    };

    use casper_engine_test_support::WasmTestBuilder;
    use casper_execution_engine::storage::global_state::in_memory::InMemoryGlobalState;

    // account seeds
    const BOB: [u8; 32] = [1u8; 32];
    const ALICE: [u8; 32] = [2u8; 32];
    const TOM: [u8; 32] = [3u8; 32];
    const LEAH: [u8; 32] = [4u8; 32];
    const JIM: [u8; 32] = [5u8; 32];
    const DANNY: [u8; 32] = [6u8; 32];

    const STAKE_CONTRACT_WASM: &str = "stake_contract.wasm";
    const TOKEN_WASM: &str = "erc20_token.wasm";
    const ERC20_TOKEN_CONTRACT_KEY: &str = "erc20_token_contract";

    const STAKE_HASH_KEY: &str = "stake_conthash";

    // contract keys
    const INIT_KEY: &str = "initialized";
    const INDEX_KEY: &str = "index";
    const KEY_NAME_DIC_STAKE_INFOS: &str = "dic_stake_infos";
    const KEY_NAME_USER_INFOS: &str = "user_infos";
    const ADMIN_KEY: &str = "admin-account";

    pub type TestContext = (
        WasmTestBuilder<InMemoryGlobalState>,
        ContractHash,
        ContractPackageHash,
        ContractHash,
        Account,
        U256,
    );

    pub type TokenContracts = [ContractHash; 2];
    pub type TestAccounts = Vec<Account>;

    pub type TestContextMultipleUsers = (
        WasmTestBuilder<InMemoryGlobalState>,
        ContractHash,
        ContractPackageHash,
        TokenContracts,
        TestAccounts,
        U256,
    );
    fn setup() -> TestContext {
        let mut builder = InMemoryWasmTestBuilder::default();
        builder.run_genesis(&*DEFAULT_RUN_GENESIS_REQUEST);

        let supply = U256::from(1_000_000_000_000_u64);

        let TOTAL_REWARD = supply.checked_div(U256::from(2));
        let install_token = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            TOKEN_WASM,
            runtime_args! {
                "name" =>String::from("TrustSwap"),
                "symbol" =>String::from("SWAP"),
                "icon" => String::from("example.png"),
                "decimals" => 9u8,
                "total_supply" => supply
            },
        )
        .build();

        // deploy the contract.
        builder.exec(install_token).commit().expect_success();

        let account = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let erc20_token = account
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");

        let erc20_token_package = account
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractPackageHash::new)
            .expect("should have contract hash");

        let install_request = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            STAKE_CONTRACT_WASM,
            runtime_args! {},
        )
        .build();

        // deploy the contract.
        builder.exec(install_request).commit().expect_success();

        let account_2 = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let stake_contract = account_2
            .named_keys()
            .get(STAKE_HASH_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");

        let stake_contract_package_key = account_2
            .named_keys()
            .get("vestor_pack")
            .expect("key not found")
            .into_hash();

        let init_request = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "init",
            runtime_args! {
                "package-hash" => stake_contract_package_key
            },
        )
        .build();

        // call init().
        builder.exec(init_request).commit().expect_success();

        // index is 0
        let mut index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 0_u64);

        // approve staking contract
        let approve = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => supply
            },
        )
        .build();

        builder.exec(approve).commit().expect_success();

        // call add_pool()
        let add_pool = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "add_pool",
            runtime_args! {
                "staking_token" => erc20_token.to_formatted_string(),
                "reward_token" => erc20_token.to_formatted_string(),
                "start_time" => 0u64,
                "end_time" => 100u64,
                "precision" => 18u64,
                "total_reward" => TOTAL_REWARD.unwrap()
            },
        )
        .build();

        builder.exec(add_pool).commit().expect_success();

        (
            builder,
            stake_contract,
            ContractPackageHash::new(stake_contract_package_key.unwrap()),
            erc20_token,
            account_2,
            supply,
        )
    }

    fn setup_multiple_users() -> TestContextMultipleUsers {
        let mut builder = InMemoryWasmTestBuilder::default();
        builder.run_genesis(&*DEFAULT_RUN_GENESIS_REQUEST);

        let supply = U256::from(1_000_000_000_000_u64);

        let TOTAL_REWARD = supply.checked_div(U256::from(2));
        let install_token = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            TOKEN_WASM,
            runtime_args! {
                "name" =>String::from("TrustSwap"),
                "symbol" =>String::from("SWAP"),
                "icon" => String::from("example.png"),
                "decimals" => 9u8,
                "total_supply" => supply
            },
        )
        .build();

        // deploy the contract.
        builder.exec(install_token).commit().expect_success();

        let account = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let erc20_token = account
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");

        let erc20_token_package = account
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractPackageHash::new)
            .expect("should have contract hash");

        let install_request = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            STAKE_CONTRACT_WASM,
            runtime_args! {},
        )
        .build();

        // deploy the contract.
        builder.exec(install_request).commit().expect_success();

        let account_2 = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let stake_contract = account_2
            .named_keys()
            .get(STAKE_HASH_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");

        let stake_contract_package_key = account_2
            .named_keys()
            .get("vestor_pack")
            .expect("key not found")
            .into_hash();

        let init_request = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "init",
            runtime_args! {
                "package-hash" => stake_contract_package_key
            },
        )
        .build();

        // call init().
        builder.exec(init_request).commit().expect_success();

        // index is 0
        let mut index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 0_u64);

        // approve staking contract
        let approve = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => supply
            },
        )
        .build();

        builder.exec(approve).commit().expect_success();

        // call add_pool()
        let add_pool = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "add_pool",
            runtime_args! {
                "staking_token" => erc20_token.to_formatted_string(),
                "reward_token" => erc20_token.to_formatted_string(),
                "start_time" => 100u64,
                "end_time" => 200u64,
                "precision" => 18u64,
                "total_reward" => TOTAL_REWARD.unwrap()
            },
        )
        .build();

        builder.exec(add_pool).commit().expect_success();

        let (acc_1, _) = AccountHash::from_bytes(&BOB).unwrap();
        let (acc_2, _) = AccountHash::from_bytes(&ALICE).unwrap();
        let (acc_3, _) = AccountHash::from_bytes(&TOM).unwrap();

        let id: Option<u64> = None;
        let transfer_1_args = runtime_args! {
            mint::ARG_TARGET => acc_1,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };
        let transfer_2_args = runtime_args! {
            mint::ARG_TARGET => acc_2,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };
        let transfer_3_args = runtime_args! {
            mint::ARG_TARGET => acc_3,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };

        let transfer_request_1 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_1_args).build();
        let transfer_request_2 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_2_args).build();
        let transfer_request_3 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_3_args).build();

        builder.exec(transfer_request_1).commit().expect_success();
        builder.exec(transfer_request_2).commit().expect_success();
        builder.exec(transfer_request_3).commit().expect_success();

        let amount = U256::from(100_000_000_000_u64);

        let tx_1 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_1),
            },
        )
        .build();

        let tx_2 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_2),
            },
        )
        .build();

        let tx_3 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_3),
            },
        )
        .build();

        let account_bob = builder
            .get_account(acc_1.to_owned())
            .expect("should have account");

        let account_alice = builder
            .get_account(acc_2.to_owned())
            .expect("should have account");

        let account_tom = builder
            .get_account(acc_3.to_owned())
            .expect("should have account");

        let test_accounts: Vec<Account> = vec![
            account_2.to_owned(),
            account_alice.to_owned(),
            account_bob.to_owned(),
            account_tom.to_owned(),
        ];

        builder.exec(tx_1).commit().expect_success();
        builder.exec(tx_2).commit().expect_success();
        builder.exec(tx_3).commit().expect_success();

        let bal_1 = get_token_balance(&account_bob, erc20_token, &builder);
        let bal_2 = get_token_balance(&account_alice, erc20_token, &builder);
        let bal_3 = get_token_balance(&account_tom, erc20_token, &builder);
        let bal_4 = get_token_balance(&account_2, erc20_token, &builder);

        assert_eq!(bal_1, amount);
        assert_eq!(bal_2, amount);
        assert_eq!(bal_3, amount);
        assert_eq!(
            bal_4,
            TOTAL_REWARD.unwrap() - amount.checked_mul(U256::from(3)).unwrap()
        );

        // approve staking contract
        let approve_1 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_1,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build();

        // approve staking contract
        let approve_2 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_2,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build(); // approve staking contract
        let approve_3 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_3,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build();

        builder.exec(approve_1).commit().expect_success();
        builder.exec(approve_2).commit().expect_success();
        builder.exec(approve_3).commit().expect_success();

        (
            builder,
            stake_contract,
            ContractPackageHash::new(stake_contract_package_key.unwrap()),
            [erc20_token, erc20_token],
            test_accounts,
            supply,
        )
    }

    fn setup_multiple_users_different_tokens() -> TestContextMultipleUsers {
        let mut builder = InMemoryWasmTestBuilder::default();
        builder.run_genesis(&*DEFAULT_RUN_GENESIS_REQUEST);

        let supply = U256::from(2_000_000_000_000_u64);

        let TOTAL_REWARD = supply.checked_div(U256::from(2));
        let install_token = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            TOKEN_WASM,
            runtime_args! {
                "name" =>String::from("TrustSwap"),
                "symbol" =>String::from("SWAP"),
                "icon" => String::from("example.png"),
                "decimals" => 9u8,
                "total_supply" => supply
            },
        )
        .build();

        let install_token_2 = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            TOKEN_WASM,
            runtime_args! {
                "name" =>String::from("Digi Dollaz"),
                "symbol" =>String::from("eDOLLAz"),
                "icon" => String::from("example.png"),
                "decimals" => 9u8,
                "total_supply" => supply
            },
        )
        .build();

        // deploy the contracts.
        builder.exec(install_token).commit().expect_success();

        let account = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let erc20_token = account
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");

        let erc20_token_package = account
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractPackageHash::new)
            .expect("should have contract hash");

        builder.exec(install_token_2).commit().expect_success();

        let mut account_2 = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let erc20_token_2 = account_2
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");

        let erc20_token_package_2 = account_2
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractPackageHash::new)
            .expect("should have contract hash");

        let install_request = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            STAKE_CONTRACT_WASM,
            runtime_args! {},
        )
        .build();

        // deploy the contract.
        builder.exec(install_request).commit().expect_success();

        account_2 = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let stake_contract = account_2
            .named_keys()
            .get(STAKE_HASH_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");

        let stake_contract_package_key = account_2
            .named_keys()
            .get("vestor_pack")
            .expect("key not found")
            .into_hash();

        let init_request = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "init",
            runtime_args! {
                "package-hash" => stake_contract_package_key
            },
        )
        .build();

        // call init().
        builder.exec(init_request).commit().expect_success();

        // index is 0
        let mut index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 0_u64);

        // approve staking contract
        let approve_main_1 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => supply
            },
        )
        .build();

        let approve_main_2 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token_2,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => supply
            },
        )
        .build();

        builder.exec(approve_main_1).commit().expect_success();
        builder.exec(approve_main_2).commit().expect_success();

        // call add_pool()
        let add_pool = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "add_pool",
            runtime_args! {
                "staking_token" => erc20_token.to_formatted_string(),
                "reward_token" => erc20_token_2.to_formatted_string(),
                "start_time" => 100u64,
                "end_time" => 200u64,
                "precision" => 36u64,
                "total_reward" => TOTAL_REWARD.unwrap()
            },
        )
        .build();

        builder.exec(add_pool).commit().expect_success();

        let (acc_1, _) = AccountHash::from_bytes(&BOB).unwrap();
        let (acc_2, _) = AccountHash::from_bytes(&ALICE).unwrap();
        let (acc_3, _) = AccountHash::from_bytes(&TOM).unwrap();
        let (acc_4, _) = AccountHash::from_bytes(&LEAH).unwrap();
        let (acc_5, _) = AccountHash::from_bytes(&JIM).unwrap();
        let (acc_6, _) = AccountHash::from_bytes(&DANNY).unwrap();

        let id: Option<u64> = None;
        let transfer_1_args = runtime_args! {
            mint::ARG_TARGET => acc_1,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };
        let transfer_2_args = runtime_args! {
            mint::ARG_TARGET => acc_2,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };
        let transfer_3_args = runtime_args! {
            mint::ARG_TARGET => acc_3,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };
        let transfer_4_args = runtime_args! {
            mint::ARG_TARGET => acc_4,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };
        let transfer_5_args = runtime_args! {
            mint::ARG_TARGET => acc_5,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };
        let transfer_6_args = runtime_args! {
            mint::ARG_TARGET => acc_6,
            mint::ARG_AMOUNT => MINIMUM_ACCOUNT_CREATION_BALANCE,
            mint::ARG_ID => id,
        };

        let transfer_request_1 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_1_args).build();
        let transfer_request_2 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_2_args).build();
        let transfer_request_3 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_3_args).build();
        let transfer_request_4 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_4_args).build();
        let transfer_request_5 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_5_args).build();
        let transfer_request_6 =
            ExecuteRequestBuilder::transfer(*DEFAULT_ACCOUNT_ADDR, transfer_6_args).build();

        builder.exec(transfer_request_1).commit().expect_success();
        builder.exec(transfer_request_2).commit().expect_success();
        builder.exec(transfer_request_3).commit().expect_success();
        builder.exec(transfer_request_4).commit().expect_success();
        builder.exec(transfer_request_5).commit().expect_success();
        builder.exec(transfer_request_6).commit().expect_success();

        let amount = U256::from(100_000_000_000_u64);

        let tx_1 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_1),
            },
        )
        .build();

        let tx_2 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_2),
            },
        )
        .build();

        let tx_3 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_3),
            },
        )
        .build();

        let tx_4 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_4),
            },
        )
        .build();

        let tx_5 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_5),
            },
        )
        .build();

        let tx_6 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            erc20_token,
            "transfer",
            runtime_args! {
                "amount" => amount,
                "recipient" => Key::Account(acc_6),
            },
        )
        .build();

        let account_bob = builder
            .get_account(acc_1.to_owned())
            .expect("should have account");

        let account_alice = builder
            .get_account(acc_2.to_owned())
            .expect("should have account");

        let account_tom = builder
            .get_account(acc_3.to_owned())
            .expect("should have account");

        let account_leah = builder
            .get_account(acc_4.to_owned())
            .expect("should have account");

        let account_jim = builder
            .get_account(acc_5.to_owned())
            .expect("should have account");

        let account_danny = builder
            .get_account(acc_6.to_owned())
            .expect("should have account");

        let test_accounts: Vec<Account> = vec![
            account_2.to_owned(),
            account_alice.to_owned(),
            account_bob.to_owned(),
            account_tom.to_owned(),
            account_leah.to_owned(),
            account_jim.to_owned(),
            account_danny.to_owned(),
        ];

        builder.exec(tx_1).commit().expect_success();
        builder.exec(tx_2).commit().expect_success();
        builder.exec(tx_3).commit().expect_success();
        builder.exec(tx_4).commit().expect_success();
        builder.exec(tx_5).commit().expect_success();
        builder.exec(tx_6).commit().expect_success();

        let bal_1 = get_token_balance(&account_bob, erc20_token, &builder);
        let bal_2 = get_token_balance(&account_alice, erc20_token, &builder);
        let bal_3 = get_token_balance(&account_tom, erc20_token, &builder);
        let bal_4 = get_token_balance(&account_leah, erc20_token, &builder);
        let bal_5 = get_token_balance(&account_jim, erc20_token, &builder);
        let bal_6 = get_token_balance(&account_danny, erc20_token, &builder);
        let bal_7 = get_token_balance(&account_2, erc20_token, &builder);

        assert_eq!(bal_1, amount);
        assert_eq!(bal_2, amount);
        assert_eq!(bal_3, amount);
        assert_eq!(bal_4, amount);
        assert_eq!(bal_5, amount);
        assert_eq!(bal_6, amount);
        assert_eq!(bal_7, supply - amount.checked_mul(U256::from(6)).unwrap());

        // approve staking contract
        let approve_1 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_1,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build();

        // approve staking contract
        let approve_2 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_2,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build(); // approve staking contract
        let approve_3 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_3,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build();

        // approve staking contract
        let approve_4 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_4,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build();

        // approve staking contract
        let approve_5 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_5,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build(); // approve staking contract
        let approve_6 = ExecuteRequestBuilder::contract_call_by_hash(
            acc_6,
            erc20_token,
            "approve",
            runtime_args! {
                "spender" => stake_contract_package_key,
                "amount" => amount
            },
        )
        .build();

        builder.exec(approve_1).commit().expect_success();
        builder.exec(approve_2).commit().expect_success();
        builder.exec(approve_3).commit().expect_success();
        builder.exec(approve_4).commit().expect_success();
        builder.exec(approve_5).commit().expect_success();
        builder.exec(approve_6).commit().expect_success();

        (
            builder,
            stake_contract,
            ContractPackageHash::new(stake_contract_package_key.unwrap()),
            [erc20_token, erc20_token_2],
            test_accounts,
            supply,
        )
    }

    #[test]
    fn should_install_erc20() {
        let mut builder = InMemoryWasmTestBuilder::default();
        builder.run_genesis(&*DEFAULT_RUN_GENESIS_REQUEST);

        let supply = U256::from(1_000_000_000_000_u64);
        let install_request = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            TOKEN_WASM,
            runtime_args! {
                "name" =>String::from("TrustSwap"),
                "symbol" =>String::from("SWAP"),
                "icon" => String::from("example.png"),
                "decimals" => 9u8,
                "total_supply" => supply
            },
        )
        .build();

        // deploy the contract.
        builder.exec(install_request).commit().expect_success();

        let account = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let erc20_token = account
            .named_keys()
            .get(ERC20_TOKEN_CONTRACT_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");
    }

    #[test]
    fn should_initialize() {
        let mut builder = InMemoryWasmTestBuilder::default();
        builder.run_genesis(&*DEFAULT_RUN_GENESIS_REQUEST);

        let install_request = ExecuteRequestBuilder::standard(
            *DEFAULT_ACCOUNT_ADDR,
            STAKE_CONTRACT_WASM,
            runtime_args! {},
        )
        .build();

        // deploy the contract.
        builder.exec(install_request).commit().expect_success();

        let account = builder
            .get_account(*DEFAULT_ACCOUNT_ADDR)
            .expect("should have account");

        let stake_contract = account
            .named_keys()
            .get(STAKE_HASH_KEY)
            .and_then(|key| key.into_hash())
            .map(ContractHash::new)
            .expect("should have contract hash");

        let mut initialized: bool = builder.get_value(stake_contract, INIT_KEY);
        assert_eq!(initialized, false);

        let stake_contract_package_key = account
            .named_keys()
            .get("vestor_pack")
            .expect("key not found")
            .into_hash();

        let init_request = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "init",
            runtime_args! {
                "package-hash" => stake_contract_package_key
            },
        )
        .build();

        // call init().
        builder.exec(init_request).commit().expect_success();

        // initialized set to true
        initialized = builder.get_value(stake_contract, INIT_KEY);
        assert_eq!(initialized, true);

        // index initialized
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 0_u64);

        // cannot call init() again
        let init_request_2 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "init",
            runtime_args! {
                "package-hash" => stake_contract_package_key
            },
        )
        .build();

        builder.exec(init_request_2).commit().expect_failure();
    }

    #[test]
    fn add_pool_same_token_works() {
        let (mut builder, stake_contract, stake_contract_package_key, erc20_token, account, supply) =
            setup();

        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let pool = get_pool(stake_contract, stake_contract_package_key, &builder);

        // verify values of input pool
        assert_eq!(pool.start_time, 0u64);
        assert_eq!(pool.end_time, 100u64);
        assert_eq!(
            pool.total_reward,
            supply.checked_div(U256::from(2)).unwrap()
        );
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        // check balance is deducted
        let balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - pool.total_reward);
    }

    #[test]
    fn add_pool_different_token_works() {
        let (
            mut builder,
            stake_contract,
            stake_contract_package_key,
            erc20_tokens,
            accounts,
            supply,
        ) = setup_multiple_users_different_tokens();

        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let pool = get_pool(stake_contract, stake_contract_package_key, &builder);

        // verify values of input pool
        assert_eq!(pool.start_time, 100u64);
        assert_eq!(pool.end_time, 200u64);
        assert_eq!(
            pool.total_reward,
            supply.checked_div(U256::from(2)).unwrap()
        );
        assert_ne!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        // check reward token balance is deducted properly
        let balance = get_token_balance(&accounts[0], erc20_tokens[1], &builder);
        assert_eq!(balance, supply - pool.total_reward);
    }

    #[test]
    fn deposit_works() {
        let (mut builder, stake_contract, stake_contract_package_key, erc20_token, account, supply) =
            setup();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 0u64);
        assert_eq!(pool.end_time, 100u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let mut balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD);

        // call deposit()
        let deposit_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => TOTAL_REWARD,
            },
        )
        .with_block_time(0)
        .build();

        builder.exec(deposit_req).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &account, &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, TOTAL_REWARD);

        balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD - TOTAL_REWARD);
        assert_eq!(user_info.amount, pool.total_staked);
    }

    #[test]
    fn deposit_more_works() {
        let (mut builder, stake_contract, stake_contract_package_key, erc20_token, account, supply) =
            setup();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 0u64);
        assert_eq!(pool.end_time, 100u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let balance_pre = get_token_balance(&account, erc20_token, &builder);

        let amount_to_deposit = U256::from(100_000_000_000u64);

        // call deposit()
        let deposit_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(0)
        .build();

        builder.exec(deposit_req).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &account, &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, amount_to_deposit);

        let mut balance = get_token_balance(&account, erc20_token, &builder);

        // subtracted balance successfully
        assert_eq!(balance, balance_pre - amount_to_deposit);
        assert_eq!(user_info.amount, pool.total_staked);

        // deposit again
        let deposit_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(50)
        .build();

        builder.exec(deposit_req_2).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &account, &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, amount_to_deposit * U256::from(2));

        balance = get_token_balance(&account, erc20_token, &builder);

        // subtracted balance successfully and
        // pending reward claimed
        assert_eq!(
            balance,
            balance_pre - (amount_to_deposit * U256::from(2))
                + TOTAL_REWARD.checked_div(U256::from(2)).unwrap()
        );
        assert_eq!(user_info.amount, pool.total_staked);
    }

    #[test]
    fn deposit_after_end_should_fail() {
        let (mut builder, stake_contract, stake_contract_package_key, erc20_token, account, supply) =
            setup();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 0u64);
        assert_eq!(pool.end_time, 100u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let mut balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD);

        // call deposit()
        let deposit_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => TOTAL_REWARD,
            },
        )
        .with_block_time(101)
        .build();

        builder.exec(deposit_req).commit().expect_failure();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);

        // verify value of pool after deposit is still 0
        assert_eq!(pool.total_staked, U256::zero());

        // check balance was not deducted
        balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD);
    }

    #[test]
    fn stop_reward_works() {
        let (mut builder, stake_contract, stake_contract_package_key, erc20_token, account, supply) =
            setup();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 0u64);
        assert_eq!(pool.end_time, 100u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let mut balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD);

        // call deposit()
        let deposit_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => TOTAL_REWARD,
            },
        )
        .with_block_time(0)
        .build();

        builder.exec(deposit_req).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &account, &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, TOTAL_REWARD);

        balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD - TOTAL_REWARD);
        assert_eq!(user_info.amount, pool.total_staked);

        let stop_reward_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "stop_reward",
            runtime_args! {
                "pool_id" => 0u64,
            },
        )
        .with_block_time(50)
        .build();

        builder.exec(stop_reward_req).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &account, &builder);

        // verify values of pool after stop_reward
        assert_eq!(pool.total_staked, TOTAL_REWARD);
        assert_eq!(pool.end_time, 50_u64);

        balance = get_token_balance(&account, erc20_token, &builder);

        // half balance returned
        assert_eq!(balance, TOTAL_REWARD / U256::from(2));
    }

    #[test]
    fn emergency_withdraw_works() {
        let (mut builder, stake_contract, stake_contract_package_key, erc20_token, account, supply) =
            setup();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 0u64);
        assert_eq!(pool.end_time, 100u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let mut balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD);

        // call deposit()
        let deposit_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => TOTAL_REWARD,
            },
        )
        .with_block_time(0)
        .build();

        builder.exec(deposit_req).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &account, &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, TOTAL_REWARD);

        balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD - TOTAL_REWARD);
        assert_eq!(user_info.amount, pool.total_staked);

        let emergency_withdraw_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "emergency_withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => TOTAL_REWARD
            },
        )
        .with_block_time(50)
        .build();

        builder
            .exec(emergency_withdraw_req)
            .commit()
            .expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &account, &builder);

        // verify values of pool after emergency withdraw
        assert_eq!(pool.total_staked, U256::zero());

        balance = get_token_balance(&account, erc20_token, &builder);

        // balance returned no reward
        assert_eq!(balance, TOTAL_REWARD);
    }

    #[test]
    fn set_admin_works() {
        let (
            mut builder,
            stake_contract,
            stake_contract_package_key,
            erc20_tokens,
            accounts,
            supply,
        ) = setup_multiple_users();

        let mut admin: AccountHash = builder.get_value(stake_contract, ADMIN_KEY);

        assert_eq!(admin, accounts[0].account_hash());

        // call set_admin()
        let mut set_admin_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "set_admin",
            runtime_args! {
                "new_admin" => accounts[1].account_hash(),
            },
        )
        .with_block_time(20)
        .build();

        builder.exec(set_admin_req).commit().expect_success();

        admin = builder.get_value(stake_contract, ADMIN_KEY);

        // new admin set successfully
        assert_eq!(admin, accounts[1].account_hash());

        // old admin no longer can call set_admin
        set_admin_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "set_admin",
            runtime_args! {
                "new_admin" => accounts[3].account_hash(),
            },
        )
        .with_block_time(21)
        .build();

        builder.exec(set_admin_req).commit().expect_failure();

        // new admin can now call set_admin
        set_admin_req = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "set_admin",
            runtime_args! {
                "new_admin" => accounts[3].account_hash(),
            },
        )
        .with_block_time(22)
        .build();

        builder.exec(set_admin_req).commit().expect_success();

        admin = builder.get_value(stake_contract, ADMIN_KEY);

        // new admin set successfully
        assert_eq!(admin, accounts[3].account_hash());
    }

    #[test]
    fn withdraw_works() {
        let (mut builder, stake_contract, stake_contract_package_key, erc20_token, account, supply) =
            setup();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 0u64);
        assert_eq!(pool.end_time, 100u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let mut balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD);

        // call deposit()
        let deposit_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => TOTAL_REWARD,
            },
        )
        .with_block_time(0)
        .build();

        builder.exec(deposit_req).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of pool after deposit
        assert_eq!(pool.total_staked, TOTAL_REWARD);

        // check balance is deducted
        balance = get_token_balance(&account, erc20_token, &builder);

        assert_eq!(balance, supply - TOTAL_REWARD - TOTAL_REWARD);

        // call withdraw() after fully finished
        let withdraw_req = ExecuteRequestBuilder::contract_call_by_hash(
            *DEFAULT_ACCOUNT_ADDR,
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => TOTAL_REWARD
            },
        )
        .with_block_time(101)
        .build();

        builder.exec(withdraw_req).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);

        // verify all tokens withdrawn
        assert_eq!(pool.total_staked, U256::zero());

        // check balance is added
        balance = get_token_balance(&account, erc20_token, &builder);

        // Should have all tokens back
        assert_eq!(balance, supply);
    }

    #[test]
    fn deposit_withdraw_multiple_users() {
        let (
            mut builder,
            stake_contract,
            stake_contract_package_key,
            erc20_tokens,
            accounts,
            supply,
        ) = setup_multiple_users();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 100u64);
        assert_eq!(pool.end_time, 200u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let mut balance = get_token_balance(&accounts[0], erc20_tokens[0], &builder);

        // balance = supply - amount allocated to accounts - amount deposited for reward
        assert_eq!(
            balance,
            supply
                - U256::from(100_000_000_000_u64)
                    .checked_mul(U256::from(3))
                    .unwrap()
                - TOTAL_REWARD
        );

        let amount_to_deposit = U256::from(100_000_000_000_u64);

        // call deposit()
        let deposit_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[0].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[3].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        builder.exec(deposit_req_1).commit().expect_success();
        builder.exec(deposit_req_2).commit().expect_success();
        builder.exec(deposit_req_3).commit().expect_success();
        builder.exec(deposit_req_4).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &accounts[0], &builder);

        // verify values of pool after deposit
        assert_eq!(
            pool.total_staked,
            amount_to_deposit.checked_mul(U256::from(4)).unwrap()
        );

        // verify user info matches amount deposited
        assert_eq!(
            user_info.amount,
            pool.total_staked.checked_div(U256::from(4)).unwrap()
        );

        // withdraw after finish
        let withdraw_req = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[0].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[3].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        builder.exec(withdraw_req).commit().expect_success();
        builder.exec(withdraw_req_2).commit().expect_success();
        builder.exec(withdraw_req_3).commit().expect_success();
        builder.exec(withdraw_req_4).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, U256::zero());

        let bal_1 = get_token_balance(&accounts[1], erc20_tokens[0], &builder);
        let bal_2 = get_token_balance(&accounts[2], erc20_tokens[0], &builder);
        let bal_3 = get_token_balance(&accounts[3], erc20_tokens[0], &builder);
        let bal_4 = get_token_balance(&accounts[0], erc20_tokens[0], &builder);

        // each user should have fair share after claiming reward
        // each user has 25%
        let amount_post_pool = amount_to_deposit + TOTAL_REWARD.checked_div(U256::from(4)).unwrap();
        assert_eq!(bal_1, amount_post_pool);
        assert_eq!(bal_2, amount_post_pool);
        assert_eq!(bal_3, amount_post_pool);
        // minter has 100 extra tokens
        assert_eq!(bal_4, amount_post_pool + amount_to_deposit);
    }

    #[test]
    fn deposit_withdraw_different_amounts() {
        let (
            mut builder,
            stake_contract,
            stake_contract_package_key,
            erc20_tokens,
            accounts,
            supply,
        ) = setup_multiple_users();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 100u64);
        assert_eq!(pool.end_time, 200u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let amount_to_deposit = U256::from(100_000_000_000_u64);

        // call deposit()
        let deposit_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit() 25% amount as accounts[1]
        let deposit_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit.checked_div(U256::from(4)).unwrap(),
            },
        )
        .with_block_time(100)
        .build();

        builder.exec(deposit_req_1).commit().expect_success();
        builder.exec(deposit_req_2).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let mut user_info = get_user_info(stake_contract, &accounts[1], &builder);

        // verify values of pool after deposit
        assert_eq!(
            pool.total_staked,
            amount_to_deposit + amount_to_deposit.checked_div(U256::from(4)).unwrap()
        );

        // verify user info matches amount deposited
        assert_eq!(user_info.amount, amount_to_deposit);

        // withdraw after finish
        let withdraw_req = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit.checked_div(U256::from(4)).unwrap(),
            },
        )
        .with_block_time(202)
        .build();

        builder.exec(withdraw_req).commit().expect_success();
        builder.exec(withdraw_req_2).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);

        // // verify values of pool after deposit
        assert_eq!(pool.total_staked, U256::zero());

        let bal_1 = get_token_balance(&accounts[1], erc20_tokens[0], &builder);
        let bal_2 = get_token_balance(&accounts[2], erc20_tokens[0], &builder);

        // all coins accounted for in user 1 + 2 balances
        assert_eq!(
            TOTAL_REWARD + amount_to_deposit + amount_to_deposit,
            bal_1 + bal_2
        );

        let total_staked = U256::from(125_000_000_000_u64);

        // 125 total tokens staked
        // accounts[1] has 100/125 share = 80%.  Therefore accounts[1]
        // should have amount_to_deposit + 80% of total reward.
        // accounts[2] has 25/125 share = 20%, so should have
        // original balance + 20% of 500 tokens.

        // 80% reward earned (4/5 shares)
        assert_eq!(
            bal_1,
            amount_to_deposit + TOTAL_REWARD.checked_div(U256::from(5)).unwrap() * U256::from(4)
        );
        // 20% reward earned (1/5 shares)
        assert_eq!(
            bal_2,
            amount_to_deposit + TOTAL_REWARD.checked_div(U256::from(5)).unwrap()
        );
    }

    #[test]
    fn deposit_withdraw_different_times() {
        let (
            mut builder,
            stake_contract,
            stake_contract_package_key,
            erc20_tokens,
            accounts,
            supply,
        ) = setup_multiple_users();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 100u64);
        assert_eq!(pool.end_time, 200u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_eq!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let amount_to_deposit = U256::from(100_000_000_000_u64);

        // call deposit()
        let deposit_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit() different time as accounts[1]
        let deposit_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(150)
        .build();

        builder.exec(deposit_req_1).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let mut user_info = get_user_info(stake_contract, &accounts[1], &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, amount_to_deposit);

        // verify user info matches amount deposited
        assert_eq!(user_info.amount, amount_to_deposit);

        // withdraw after finish
        let withdraw_req = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(150)
        .build();

        let withdraw_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(201)
        .build();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        user_info = get_user_info(stake_contract, &accounts[1], &builder);

        // account 1 deposits 100 tokens at 150 and claims
        // half of the reward (250)
        builder.exec(withdraw_req).commit().expect_success();
        // account 2 deposits 100 tokens at 150
        builder.exec(deposit_req_2).commit().expect_success();

        // verify mid-point balances
        let mut bal_1 = get_token_balance(&accounts[1], erc20_tokens[0], &builder);
        let mut bal_2 = get_token_balance(&accounts[2], erc20_tokens[0], &builder);
        assert_eq!(bal_1, amount_to_deposit + TOTAL_REWARD / U256::from(2));
        assert_eq!(bal_2, U256::zero());

        // account 2 withdraws 100 tokens at 150 and claims
        // half of the reward (250)
        builder.exec(withdraw_req_2).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        bal_1 = get_token_balance(&accounts[1], erc20_tokens[0], &builder);
        bal_2 = get_token_balance(&accounts[2], erc20_tokens[0], &builder);

        // // verify values of pool after deposit
        assert_eq!(pool.total_staked, U256::zero());

        let bal_1 = get_token_balance(&accounts[1], erc20_tokens[0], &builder);
        let bal_2 = get_token_balance(&accounts[2], erc20_tokens[0], &builder);

        println!("BAL_1: {:?}", bal_1);
        println!("BAL_2: {:?}", bal_2);

        let total_staked = amount_to_deposit * U256::from(2);

        // all tokens accounted for
        assert_eq!(total_staked + TOTAL_REWARD, bal_1 + bal_2);

        // 50% reward earned (1/1 shares) for 50% duration
        assert_eq!(
            bal_1,
            amount_to_deposit + TOTAL_REWARD.checked_div(U256::from(2)).unwrap()
        );
        // 50% reward earned (1/1 shares) for 50% duration
        assert_eq!(
            bal_2,
            amount_to_deposit + TOTAL_REWARD.checked_div(U256::from(2)).unwrap()
        );
    }

    #[test]
    fn deposit_withdraw_6_users_tokens_ne() {
        let (
            mut builder,
            stake_contract,
            stake_contract_package_key,
            erc20_tokens,
            accounts,
            supply,
        ) = setup_multiple_users_different_tokens();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 100u64);
        assert_eq!(pool.end_time, 200u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_ne!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let amount_to_deposit = U256::from(100_000_000_000_u64);

        // call deposit()
        let deposit_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[3].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[4].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_5 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_6 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[6].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        builder.exec(deposit_req_1).commit().expect_success();
        builder.exec(deposit_req_2).commit().expect_success();
        builder.exec(deposit_req_3).commit().expect_success();
        builder.exec(deposit_req_4).commit().expect_success();
        builder.exec(deposit_req_5).commit().expect_success();
        builder.exec(deposit_req_6).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &accounts[1], &builder);

        // verify values of pool after deposit
        assert_eq!(
            pool.total_staked,
            amount_to_deposit.checked_mul(U256::from(6)).unwrap()
        );

        // verify user info matches amount deposited
        assert_eq!(
            user_info.amount,
            pool.total_staked.checked_div(U256::from(6)).unwrap()
        );

        // withdraw after finish
        let withdraw_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[3].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[4].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_5 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_6 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[6].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        builder.exec(withdraw_req_1).commit().expect_success();
        builder.exec(withdraw_req_2).commit().expect_success();
        builder.exec(withdraw_req_3).commit().expect_success();
        builder.exec(withdraw_req_4).commit().expect_success();
        builder.exec(withdraw_req_5).commit().expect_success();
        builder.exec(withdraw_req_6).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, U256::zero());

        let bal_1 = get_token_balance(&accounts[1], erc20_tokens[1], &builder);
        let bal_2 = get_token_balance(&accounts[2], erc20_tokens[1], &builder);
        let bal_3 = get_token_balance(&accounts[3], erc20_tokens[1], &builder);
        let bal_4 = get_token_balance(&accounts[4], erc20_tokens[1], &builder);
        let bal_5 = get_token_balance(&accounts[5], erc20_tokens[1], &builder);
        let bal_6 = get_token_balance(&accounts[6], erc20_tokens[1], &builder);

        // each user should have fair share after claiming reward
        // each user has 1/6
        let amount_post_pool = TOTAL_REWARD.checked_div(U256::from(6)).unwrap();
        assert_eq!(bal_1, amount_post_pool);
        assert_eq!(bal_2, amount_post_pool);
        assert_eq!(bal_3, amount_post_pool);
        assert_eq!(bal_4, amount_post_pool);
        assert_eq!(bal_5, amount_post_pool);
        assert_eq!(bal_6, amount_post_pool);
    }

    // 6 accounts deposit and withdraw
    // different amounts at different times
    #[test]
    fn live_scenario_test_1() {
        let (
            mut builder,
            stake_contract,
            stake_contract_package_key,
            erc20_tokens,
            accounts,
            supply,
        ) = setup_multiple_users_different_tokens();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 100u64);
        assert_eq!(pool.end_time, 200u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_ne!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let amount_to_deposit = U256::from(100_000_000_000_u64);

        // DEPOSIT REQUESTS

        // call deposit()
        let deposit_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(2),
            },
        )
        .with_block_time(115)
        .build();

        // call deposit()
        let deposit_req_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[3].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(4),
            },
        )
        .with_block_time(134)
        .build();

        // call deposit()
        let deposit_req_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[4].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(125)
        .build();

        // call deposit()
        let deposit_req_5 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(10),
            },
        )
        .with_block_time(190)
        .build();

        // call deposit()
        let deposit_req_6 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[6].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // WITHDRAW REQUESTS
        // withdraw after finish
        let withdraw_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(2)
            },
        )
        .with_block_time(178)
        .build();

        let withdraw_req_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[3].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(4)
            },
        )
        .with_block_time(144)
        .build();

        let withdraw_req_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[4].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_5 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(10)
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_6 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[6].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        // EXECUTION REQUESTS IN ORDER

        // TOKEN AMOUNT - TIMESTAMP
        // 100 - 100
        builder.exec(deposit_req_1).commit().expect_success();
        // 100 - 100
        builder.exec(deposit_req_6).commit().expect_success();
        // 50 - 115
        builder.exec(deposit_req_2).commit().expect_success();
        // 100 - 125
        builder.exec(deposit_req_4).commit().expect_success();
        // 25 - 134
        builder.exec(deposit_req_3).commit().expect_success();
        // 50 - 144
        builder.exec(withdraw_req_3).commit().expect_success();
        // 100 - 178
        builder.exec(withdraw_req_2).commit().expect_success();
        // 10 - 190
        builder.exec(deposit_req_5).commit().expect_success();
        // 100 - 201
        builder.exec(withdraw_req_1).commit().expect_success();
        // 10 - 201
        builder.exec(withdraw_req_5).commit().expect_success();
        // 100 - 201
        builder.exec(withdraw_req_6).commit().expect_success();
        // 25 - 201
        builder.exec(withdraw_req_4).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &accounts[1], &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, U256::zero());

        let bal_1 = get_token_balance(&accounts[1], erc20_tokens[1], &builder);
        let bal_2 = get_token_balance(&accounts[2], erc20_tokens[1], &builder);
        let bal_3 = get_token_balance(&accounts[3], erc20_tokens[1], &builder);
        let bal_4 = get_token_balance(&accounts[4], erc20_tokens[1], &builder);
        let bal_5 = get_token_balance(&accounts[5], erc20_tokens[1], &builder);
        let bal_6 = get_token_balance(&accounts[6], erc20_tokens[1], &builder);

        // accounts 1 and 6 both had same amount + timeframe
        assert_eq!(bal_1, bal_6);

        let total_balance_earned = bal_1 + bal_2 + bal_3 + bal_4 + bal_5 + bal_6;

        // allow 1e-6 of dust
        assert!(total_balance_earned >= TOTAL_REWARD - U256::from(1_000));
    }

    // same as live scenario test 1 with
    // deposit and withdraw multiple times from
    // same accounts sprinkled in along with a few
    // failed transaction requests.
    #[test]
    fn live_scenario_test_2() {
        let (
            mut builder,
            stake_contract,
            stake_contract_package_key,
            erc20_tokens,
            accounts,
            supply,
        ) = setup_multiple_users_different_tokens();

        let TOTAL_REWARD = supply.checked_div(U256::from(2)).unwrap();
        // index is now 1
        let index: u64 = builder.get_value(stake_contract, INDEX_KEY);
        assert_eq!(index, 1_u64);

        let mut pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        // verify values of input pool
        assert_eq!(pool.start_time, 100u64);
        assert_eq!(pool.end_time, 200u64);
        assert_eq!(pool.total_reward, TOTAL_REWARD);
        assert_ne!(pool.reward_token, pool.staking_token);
        assert_eq!(pool.owner, *DEFAULT_ACCOUNT_ADDR);

        let amount_to_deposit = U256::from(100_000_000_000_u64);

        // DEPOSIT REQUESTS

        // call deposit()
        let deposit_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // call deposit()
        let deposit_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(2),
            },
        )
        .with_block_time(115)
        .build();

        let deposit_req_2_again = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(2),
            },
        )
        .with_block_time(145)
        .build();

        // call deposit()
        let deposit_req_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[3].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(4),
            },
        )
        .with_block_time(134)
        .build();

        // call deposit()
        let deposit_req_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[4].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(125)
        .build();

        // call deposit()
        let deposit_req_5 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(10),
            },
        )
        .with_block_time(190)
        .build();
        let deposit_req_5_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(10),
            },
        )
        .with_block_time(192)
        .build();
        let deposit_req_5_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(10),
            },
        )
        .with_block_time(193)
        .build();
        let deposit_req_5_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(10),
            },
        )
        .with_block_time(194)
        .build();
        let deposit_req_5_5 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(10),
            },
        )
        .with_block_time(195)
        .build();
        let deposit_req_5_6_err = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(196)
        .build();

        // call deposit()
        let deposit_req_6 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[6].account_hash(),
            stake_contract,
            "deposit",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit,
            },
        )
        .with_block_time(100)
        .build();

        // WITHDRAW REQUESTS
        // withdraw after finish
        let withdraw_req_1 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[1].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_2 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(2)
            },
        )
        .with_block_time(178)
        .build();

        let withdraw_req_2_again = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(4)
            },
        )
        .with_block_time(188)
        .build();

        let withdraw_req_2_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[2].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(4)
            },
        )
        .with_block_time(206)
        .build();

        let withdraw_req_3 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[3].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(4)
            },
        )
        .with_block_time(144)
        .build();

        let withdraw_req_4 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[4].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_5 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[5].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit / U256::from(2)
            },
        )
        .with_block_time(201)
        .build();

        let withdraw_req_6 = ExecuteRequestBuilder::contract_call_by_hash(
            accounts[6].account_hash(),
            stake_contract,
            "withdraw",
            runtime_args! {
                "pool_id" => 0u64,
                "amount" => amount_to_deposit
            },
        )
        .with_block_time(201)
        .build();

        // EXECUTION REQUESTS IN ORDER

        // AMOUNT - TIMESTAMP
        // 100 - 100
        builder.exec(deposit_req_1).commit().expect_success();
        // 100 - 100
        builder.exec(deposit_req_6).commit().expect_success();
        // 50 - 115
        builder.exec(deposit_req_2).commit().expect_success();
        // 100 - 125
        builder.exec(deposit_req_4).commit().expect_success();
        // 25 - 134
        builder.exec(deposit_req_3).commit().expect_success();
        // 50 - 144
        builder.exec(withdraw_req_3).commit().expect_success();

        // account 2 deposits 50 more
        // 50 - 145
        builder.exec(deposit_req_2_again).commit().expect_success();

        // account 2 withdraws 25 in 2 tx
        // 25 - 178
        builder.exec(withdraw_req_2).commit().expect_success();
        // 25 - 188
        builder.exec(withdraw_req_2_again).commit().expect_success();

        // 6 deposits from same account - last is error (out of balance)
        // 10 - 190
        builder.exec(deposit_req_5).commit().expect_success();
        // 10 - 192
        builder.exec(deposit_req_5_2).commit().expect_success();
        // 10 - 193
        builder.exec(deposit_req_5_3).commit().expect_success();
        // 10 - 194
        builder.exec(deposit_req_5_4).commit().expect_success();
        // 10 - 195
        builder.exec(deposit_req_5_5).commit().expect_success();
        // 100 - 196
        builder.exec(deposit_req_5_6_err).commit().expect_failure();

        // 100 - 201
        builder.exec(withdraw_req_1).commit().expect_success();
        // 50 - 201
        builder.exec(withdraw_req_5).commit().expect_success();
        // 100 - 201
        builder.exec(withdraw_req_6).commit().expect_success();
        // 25 - 201
        builder.exec(withdraw_req_4).commit().expect_success();
        // 25 - 206
        builder.exec(withdraw_req_2_3).commit().expect_success();

        pool = get_pool(stake_contract, stake_contract_package_key, &builder);
        let user_info = get_user_info(stake_contract, &accounts[1], &builder);

        // verify values of pool after deposit
        assert_eq!(pool.total_staked, U256::zero());

        let bal_1 = get_token_balance(&accounts[1], erc20_tokens[1], &builder);
        let bal_2 = get_token_balance(&accounts[2], erc20_tokens[1], &builder);
        let bal_3 = get_token_balance(&accounts[3], erc20_tokens[1], &builder);
        let bal_4 = get_token_balance(&accounts[4], erc20_tokens[1], &builder);
        let bal_5 = get_token_balance(&accounts[5], erc20_tokens[1], &builder);
        let bal_6 = get_token_balance(&accounts[6], erc20_tokens[1], &builder);

        // accounts 1 and 6 both had same amount + timeframe
        assert_eq!(bal_1, bal_6);

        let total_balance_earned = bal_1 + bal_2 + bal_3 + bal_4 + bal_5 + bal_6;

        // allow 1e-6 of dust
        assert!(total_balance_earned >= TOTAL_REWARD - U256::from(1_000));
    }

    fn make_pool_key_id(pool_id: u64, conthash: ContractPackageHash) -> String {
        let c_hash = conthash.to_string();
        let (pre_key, _) = c_hash.split_at(15);
        let append = pool_id.to_string();

        String::from(pre_key) + &append
    }

    fn make_user_key_by_id(account: Account, pool_id: u64) -> String {
        let account_hash = account.account_hash().to_string();
        let (pre_key, _) = account_hash.split_at(15);
        let append = pool_id.to_string();

        String::from(pre_key) + &append
    }

    fn get_pool(
        stake_contract: ContractHash,
        stake_contract_package_key: ContractPackageHash,
        builder: &WasmTestBuilder<InMemoryGlobalState>,
    ) -> StakePool {
        let contract_keys: NamedKeys = builder
            .to_owned()
            .get_contract(stake_contract)
            .unwrap()
            .take_named_keys();

        let seed_uref: Key = *contract_keys.get(KEY_NAME_DIC_STAKE_INFOS).unwrap();

        let pool_key = make_pool_key_id(0_u64, stake_contract_package_key);

        let binding: StoredValue = builder
            .query_dictionary_item(None, *seed_uref.as_uref().unwrap(), &pool_key)
            .expect("Doesn't exist");

        let g = binding.as_cl_value().unwrap().inner_bytes();

        // cut off CLType bytes
        let pool_bytes = &g[4..g.len()];

        // unpack pool into StakePool
        StakePool::unpack(pool_bytes.to_vec())
    }

    fn get_user_info(
        stake_contract: ContractHash,
        account: &Account,
        builder: &WasmTestBuilder<InMemoryGlobalState>,
    ) -> UserInfo {
        let contract_keys: NamedKeys = builder
            .to_owned()
            .get_contract(stake_contract)
            .unwrap()
            .take_named_keys();

        let seed_uref: Key = *contract_keys.get(KEY_NAME_USER_INFOS).unwrap();

        let user_key = make_user_key_by_id(account.to_owned(), 0_u64);

        let binding: StoredValue = builder
            .query_dictionary_item(None, *seed_uref.as_uref().unwrap(), &user_key)
            .expect("Doesn't exist");

        let g = binding.as_cl_value().unwrap().inner_bytes();

        // cut off CLType bytes
        let info_bytes = &g[4..g.len()];

        // unpack pool into StakePool
        UserInfo::unpack(info_bytes.to_vec())
    }

    pub fn get_token_balance(
        account: &Account,
        token_hash: ContractHash,
        builder: &WasmTestBuilder<InMemoryGlobalState>,
    ) -> U256 {
        // Check balances
        let bal_key = base64::encode(Address::from(account.account_hash()).to_bytes().unwrap());

        let erc20_keys: NamedKeys = builder.get_contract(token_hash).unwrap().take_named_keys();

        let bal_uref: Key = *erc20_keys.get("balances").unwrap();

        let mut b: StoredValue = builder
            .query_dictionary_item(None, *bal_uref.as_uref().unwrap(), &bal_key)
            .expect("Doesn't exist");

        let mut balance = b.as_cl_value();
        let mut bal = balance.to_owned().unwrap();
        bal.to_owned().into_t::<U256>().unwrap()
    }

    //===============================================================
    //
    //
    //
    //                     SEPERATOR
    //
    //
    //
    //
    //
    //
    //                     SEPERATOR
    //
    //
    //
    //
    //
    //
    //
    //
    //                     SEPERATOR
    //
    //
    //
    //
    //===============================================================

    #[derive(Clone, Copy, Debug)]
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

            for i in 0..32 {
                res.push(amount_bytes[i])
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

    #[derive(Debug)]
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
            let id_bytes = self.id.into_bytes().unwrap();
            let last_reward_timestamp = self.last_reward_timestamp.into_bytes().unwrap();
            let staking_token_bytes = self.staking_token.as_bytes();
            let reward_token_bytes = self.reward_token.as_bytes();
            let start_time_bytes = self.start_time.into_bytes().unwrap();
            let end_time_bytes = self.end_time.into_bytes().unwrap();
            let precision_bytes = self.precision.into_bytes().unwrap();
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
        pub fn unpack_owner(src: &Vec<u8>) -> [u8; 32] {
            let owner: [u8; 32] = src[104..136].try_into().unwrap();
            owner
        }
    }

    /// An enum representing an [`AccountHash`] or a [`ContractPackageHash`].
    #[derive(PartialOrd, Ord, PartialEq, Eq, Hash, Clone, Copy, Debug)]
    pub enum Address {
        /// Represents an account hash.
        Account(AccountHash),
        /// Represents a contract package hash.
        Contract(ContractPackageHash),
    }

    impl Address {
        /// Returns the inner account hash if `self` is the `Account` variant.
        pub fn as_account_hash(&self) -> Option<&AccountHash> {
            if let Self::Account(v) = self {
                Some(v)
            } else {
                None
            }
        }

        /// Returns the inner contract hash if `self` is the `Contract` variant.
        pub fn as_contract_package_hash(&self) -> Option<&ContractPackageHash> {
            if let Self::Contract(v) = self {
                Some(v)
            } else {
                None
            }
        }
    }

    impl From<ContractPackageHash> for Address {
        fn from(contract_package_hash: ContractPackageHash) -> Self {
            Self::Contract(contract_package_hash)
        }
    }

    impl From<AccountHash> for Address {
        fn from(account_hash: AccountHash) -> Self {
            Self::Account(account_hash)
        }
    }

    impl From<Address> for Key {
        fn from(address: Address) -> Self {
            match address {
                Address::Account(account_hash) => Key::Account(account_hash),
                Address::Contract(contract_package_hash) => {
                    Key::Hash(contract_package_hash.value())
                }
            }
        }
    }

    impl CLTyped for Address {
        fn cl_type() -> casper_types::CLType {
            CLType::Key
        }
    }

    impl ToBytes for Address {
        fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
            Key::from(*self).to_bytes()
        }

        fn serialized_length(&self) -> usize {
            Key::from(*self).serialized_length()
        }
    }

    impl FromBytes for Address {
        fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
            let (key, remainder) = Key::from_bytes(bytes)?;

            let address = match key {
                Key::Account(account_hash) => Address::Account(account_hash),
                Key::Hash(raw_contract_package_hash) => {
                    let contract_package_hash = ContractPackageHash::new(raw_contract_package_hash);
                    Address::Contract(contract_package_hash)
                }
                _ => return Err(bytesrepr::Error::Formatting),
            };

            Ok((address, remainder))
        }
    }
}
