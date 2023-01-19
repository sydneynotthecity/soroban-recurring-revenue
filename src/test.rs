#![cfg(test)]

use super::*;

use soroban_sdk::{
    testutils::{Accounts, Ledger, LedgerInfo},
    xdr::Asset,
    Env,
};

use crate::token::Client as TokenClient;

/// The first test function, `test_valid_sequence()`, we test the contract
/// running in the sequence that is expected: sender approves on the token
/// contract, sender initializes the RecurringRevenueContract, and receiver makes some
/// withdraws. Along the way, we check allowances and balances.
#[test]
fn test_valid_sequence() {
    // Just like always, we (say it with me) register the RecurringRevenueContract
    // contract in a default Soroban environment, and build a client that can be
    // used to invoke the contract.
    let env = Env::default();
    let contract_id = env.register_contract(None, RecurringRevenueContract);
    let client = RecurringRevenueContractClient::new(&env, &contract_id);

    // For this contract, we'll need to set some ledger state to test against.
    // If you do the math, you can tell when we wrote this test!
    env.ledger().set(LedgerInfo {
        timestamp: 1669726145,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    // We create two user accounts to test with, `u1` and `u2`
    let u1 = env.accounts().generate(); // `Sender` account
    let u2 = env.accounts().generate_and_create(); // `Receiver` account

    env.accounts().update_balance(&u1, 1_000_000_000);

    // We register a token contract that we can use to test our allowance and
    // payments. For testing purposes, the specific `contract_id` we use doesn't
    // really matter.
    // let asset_native = Asset::Native;

    let id = env.register_stellar_asset_contract(Asset::Native);
    // let id = env.register_contract_token(&BytesN::from_array(
    //     &env,
    //     &[
    //         78, 52, 121, 202, 209, 66, 106, 25, 193, 181, 10, 91, 46, 213, 58, 244, 217, 115, 23,
    //         232, 144, 71, 210, 113, 57, 46, 203, 166, 210, 20, 155, 105,
    //     ],
    // ));

    // We create a client that can be used for our token contract and we invoke
    // the `init` function. Again, in tests, the values we supply here are
    // inconsequential.
    let token = TokenClient::new(&env, &id);
    // token.init(
    //     &Identifier::Account(u1.clone()),
    //     &token::TokenMetadata {
    //         name: "USD coin".into_val(&env),
    //         symbol: "USDC".into_val(&env),
    //         decimals: 7,
    //     },
    // );

    // // We use the `u1` account to mint 1,000,000,000 Stroops of our token (that
    // // is equal to 100 units of the asset).
    // token.with_source_account(&u1).mint(
    //     &Signature::Invoker,
    //     &0,
    //     &Identifier::Account(u1.clone()),
    //     &1000000000,
    // );

    // We invoke the token contract's `approve` function as the `u1` account,
    // allowing our AllowanceContract to spend tokens out of the `u1` balance.
    // We are giving the contract a 500,000,000 Stroop (== 50 units) allowance.
    token.with_source_account(&u1).incr_allow(
        &Signature::Invoker,
        &0,
        &Identifier::Contract(contract_id.clone()),
        &5000000000,
    );

    // We invoke the token contract's `allowance` function to ensure everything
    // has worked up to this point.
    assert_eq!(
        token.allowance(
            &Identifier::Account(u1.clone()),
            &Identifier::Contract(contract_id),
        ),
        5000000000
    );

    // We invoke the `init` function of the RecurringRevenueContract, providing the
    // starting arguments. These values result in a weekly payment of
    // 10,000,000 stroops, or 10 XLM
    client
        .with_source_account(&u1)
        .init(&u2, &id, &1669593600, &10000000, &(7 * 24 * 60 * 60));

    // We set new ledger state to simulate time passing. Here, we have increased
    // the timestamp by one second.
    env.ledger().set(LedgerInfo {
        timestamp: 1669726146,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    // We invoke the inaugural `withdraw` to get the first payment paid out.
    // Then, we make sure the `u2` account's token balance has increased to
    // 10,000,000. Note again we don't need any signature here to invoke the
    // `withdraw` function.
    client.withdraw();
    assert_eq!(token.balance(&Identifier::Account(u2.clone())), 10000000);

    // // We (again) set new ledger state to simulate time passing. This time,
    // // we've increased the timestamp by one week and one second.
    // env.ledger().set(LedgerInfo {
    //     timestamp: 1669726146 + (7 * 24 * 60 * 60) + 1,
    //     protocol_version: 1,
    //     sequence_number: 10,
    //     network_passphrase: Default::default(),
    //     base_reserve: 10,
    // });

    // // We invoke `withdraw` again, and check that the `u2` token balance
    // // reflects two payment transfers.
    // client.withdraw();
    // assert_eq!(token.balance(&Identifier::Account(u2.clone())), 10000000 * 2);

    // // A third time, we set new ledger state to simulate time passing. Here, we
    // // skip ahead two weeks and two seconds from the `init` invocation.
    // env.ledger().set(LedgerInfo {
    //     timestamp: 1669726146 + (7 * 24 * 60 * 60) + 1 + (7 * 24 * 60 * 60) + 1,
    //     protocol_version: 1,
    //     sequence_number: 10,
    //     network_passphrase: Default::default(),
    //     base_reserve: 10,
    // });

    // // We invoke `withdraw` again, and check that the `u2` token balance now
    // // reflects three payment transfers.
    // client.withdraw();
    // assert_eq!(token.balance(&Identifier::Account(u2.clone())), 10000000 * 3);
}

/// In our next test function, `test_invalid_sequence()`, we are testing the
/// case where things are setup in the same way, but the a second `withdraw`
/// invocation is made too quickly.
#[test]
#[should_panic(expected = "Status(ContractError(4))")] // We want this test to panic since it is withdrawing too quickly.
fn test_invalid_sequence() {
    // Almost everything in this test is identical to the previous one. We'll
    // drop a comment to let you know when things are getting interesting again.
    let env = Env::default();
    let u1 = env.accounts().generate();
    let u2 = env.accounts().generate_and_create();

    env.accounts().update_balance(&u1, 1_000_000_000);

    env.ledger().set(LedgerInfo {
        timestamp: 1669726145,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    let contract_id = env.register_contract(None, RecurringRevenueContract);
    let client = RecurringRevenueContractClient::new(&env, &contract_id);

    let id = env.register_stellar_asset_contract(Asset::Native);
    // let id = env.register_contract_token(&BytesN::from_array(
    //     &env,
    //     &[
    //         78, 52, 121, 202, 209, 66, 106, 25, 193, 181, 10, 91, 46, 213, 58, 244, 217, 115, 23,
    //         232, 144, 71, 210, 113, 57, 46, 203, 166, 210, 20, 155, 105,
    //     ],
    // ));

    let token = TokenClient::new(&env, &id);
    // token.init(
    //     &Identifier::Account(u1.clone()),
    //     &token::TokenMetadata {
    //         name: "USD coin".into_val(&env),
    //         symbol: "USDC".into_val(&env),
    //         decimals: 7,
    //     },
    // );

    // token.with_source_account(&u1).mint(
    //     &Signature::Invoker,
    //     &0,
    //     &Identifier::Account(u1.clone()),
    //     &1000000000,
    // );

    token.with_source_account(&u1).incr_allow(
        &Signature::Invoker,
        &0,
        &Identifier::Contract(contract_id.clone()),
        &500000000,
    );

    assert_eq!(
        token.allowance(
            &Identifier::Account(u1.clone()),
            &Identifier::Contract(contract_id),
        ),
        500000000
    );

    client
        .with_source_account(&u1)
        .init(&u2, &id, &1669593600, &10000000, &(7 * 24 * 60 * 60));

    env.ledger().set(LedgerInfo {
        timestamp: 1669726146,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    client.withdraw();
    assert_eq!(token.balance(&Identifier::Account(u2.clone())), 10000000);

    env.ledger().set(LedgerInfo {
        timestamp: 1669726146 + (7 * 24 * 60 * 60) + 1,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    client.withdraw();
    assert_eq!(
        token.balance(&Identifier::Account(u2.clone())),
        10000000 * 2
    );

    // Ok, stop here! This time, for our third `withdraw` invocation, we are
    // only adding 20 seconds to the previous invocation. Since we've set up for
    // weekly allowance transfers, this attempt should fail.
    env.ledger().set(LedgerInfo {
        timestamp: 1669726146 + (7 * 24 * 60 * 60) + 1 + 20,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    // We don't need an assertion here, since this invocation should fail and
    // respond with `Status(ContractError(4))`.
    client.withdraw();
}

/// In our next test function, `test_invalid_init()`, we test to make sure that
/// invoking the RecurringRevenueContract `init` function with invalid arguments will
/// fail as expected. Specifically, we are passing `0` for the `step` value.
#[test]
#[should_panic(expected = "Status(ContractError(7))")] // We want this test to panic since we are giving an unusable argument.
fn test_invalid_init() {
    // Almost everything in this test is identical to the first one. We'll drop
    // a comment to let you know when things are getting interesting again.
    let env = Env::default();
    env.ledger().set(LedgerInfo {
        timestamp: 1669726145,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    let u1 = env.accounts().generate();
    let u2 = env.accounts().generate_and_create();

    env.accounts().update_balance(&u1, 1_000_000_000);

    let contract_id = env.register_contract(None, RecurringRevenueContract);
    let client = RecurringRevenueContractClient::new(&env, &contract_id);

    let id = env.register_stellar_asset_contract(Asset::Native);
    // let id = env.register_contract_token(&BytesN::from_array(
    //     &env,
    //     &[
    //         78, 52, 121, 202, 209, 66, 106, 25, 193, 181, 10, 91, 46, 213, 58, 244, 217, 115, 23,
    //         232, 144, 71, 210, 113, 57, 46, 203, 166, 210, 20, 155, 105,
    //     ],
    // ));

    let token = TokenClient::new(&env, &id);
    // token.init(
    //     &Identifier::Account(u1.clone()),
    //     &token::TokenMetadata {
    //         name: "USD coin".into_val(&env),
    //         symbol: "USDC".into_val(&env),
    //         decimals: 7,
    //     },
    // );

    // token.with_source_account(&u1).mint(
    //     &Signature::Invoker,
    //     &0,
    //     &Identifier::Account(u1.clone()),
    //     &1000000000,
    // );

    token.with_source_account(&u1).incr_allow(
        &Signature::Invoker,
        &0,
        &Identifier::Contract(contract_id.clone()),
        &500000000,
    );

    assert_eq!(
        token.allowance(
            &Identifier::Account(u1.clone()),
            &Identifier::Contract(contract_id),
        ),
        500000000
    );

    // Ok, stop here! This time, when invoking `init`, we give a `0` for the
    // `step` field. This isn't possible because it would turn the
    // allowance-dripping faucet into a rusted old faucet that has been welded
    // shut. Also, dividing by zero is impossible. So, that's an important
    // consideration, too.
    client.with_source_account(&u1).init(
        &u2,         // our `receiver` account
        &id,         // our token contract id
        &1669593600, // start epoch for the payments
        &10000000,   // payment amount of 10XLM
        &0,          // 0 withdraw per second (why would you even do this?)
    );

    // Again, there's no need for an assertion here, since this invocation
    // should fail and respond with `Status(ContractError(7))`.
}

/// In our final test function, `test_invalid_premature_withdrawal()`, we test to make sure that
/// the receiver cannot prematurely withdraw funds from the RecurringRevenueContract.
/// The contract will init() as expected, but the receiver will be unable to withdraw funds
/// because they are too early!
#[test]
#[should_panic(expected = "Status(ContractError(5))")] // We want this test to panic since we are giving an unusable argument.
fn test_invalid_init_withdrawal() {
    // Almost everything in this test is identical to the first one. We'll drop
    // a comment to let you know when things are getting interesting again.
    let env = Env::default();
    env.ledger().set(LedgerInfo {
        timestamp: 1669726145,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    let u1 = env.accounts().generate();
    let u2 = env.accounts().generate_and_create();

    env.accounts().update_balance(&u1, 1_000_000_000);

    let contract_id = env.register_contract(None, RecurringRevenueContract);
    let client = RecurringRevenueContractClient::new(&env, &contract_id);

    let id = env.register_stellar_asset_contract(Asset::Native);
    // let id = env.register_contract_token(&BytesN::from_array(
    //     &env,
    //     &[
    //         78, 52, 121, 202, 209, 66, 106, 25, 193, 181, 10, 91, 46, 213, 58, 244, 217, 115, 23,
    //         232, 144, 71, 210, 113, 57, 46, 203, 166, 210, 20, 155, 105,
    //     ],
    // ));

    let token = TokenClient::new(&env, &id);
    // token.init(
    //     &Identifier::Account(u1.clone()),
    //     &token::TokenMetadata {
    //         name: "USD coin".into_val(&env),
    //         symbol: "USDC".into_val(&env),
    //         decimals: 7,
    //     },
    // );

    // token.with_source_account(&u1).mint(
    //     &Signature::Invoker,
    //     &0,
    //     &Identifier::Account(u1.clone()),
    //     &1000000000,
    // );

    token.with_source_account(&u1).incr_allow(
        &Signature::Invoker,
        &0,
        &Identifier::Contract(contract_id.clone()),
        &500000000,
    );

    assert_eq!(
        token.allowance(
            &Identifier::Account(u1.clone()),
            &Identifier::Contract(contract_id),
        ),
        500000000
    );

    // Notice that the start epoch is much further in the future
    client.with_source_account(&u1).init(
        &u2,         // our `receiver` account
        &id,         // our token contract id
        &1701129600, // Future date
        &10000000,
        &(7 * 24 * 60 * 60), // 1 withdraw per second
    );

    client.withdraw();

    // Again, there's no need for an assertion here, since this invocation
    // should fail and respond with `Status(ContractError(6))`.
}
