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

    let id = env.register_stellar_asset_contract(Asset::Native);

    // We create a client that can be used for our token contract and we invoke
    // the `init` function. Again, in tests, the values we supply here are
    // inconsequential.
    let token = TokenClient::new(&env, &id);

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

    // We (again) set new ledger state to simulate time passing. This time,
    // we've increased the timestamp by one week and one second.
    env.ledger().set(LedgerInfo {
        timestamp: 1669726146 + (7 * 24 * 60 * 60) + 1,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    // We invoke `withdraw` again, and check that the `u2` token balance
    // reflects two payment transfers.
    client.with_source_account(&u2).withdraw();
    assert_eq!(token.balance(&Identifier::Account(u2.clone())), 10000000 * 2);

    // A third time, we set new ledger state to simulate time passing. Here, we
    // skip ahead two weeks and two seconds from the `init` invocation.
    env.ledger().set(LedgerInfo {
        timestamp: 1669726146 + (7 * 24 * 60 * 60) + 1 + (7 * 24 * 60 * 60) + 1,
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    });

    // We invoke `withdraw` again, and check that the `u2` token balance now
    // reflects three payment transfers.
    client.withdraw();
    assert_eq!(token.balance(&Identifier::Account(u2.clone())), 10000000 * 3);
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

    let token = TokenClient::new(&env, &id);

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

    let token = TokenClient::new(&env, &id);

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

/// This test function, `test_invalid_premature_withdrawal()`, we test to make sure that
/// the receiver cannot prematurely withdraw funds from the RecurringRevenueContract.
/// The contract will init() as expected, but the receiver will be unable to withdraw funds
/// because they are too early!
#[test]
#[should_panic(expected = "Status(ContractError(5))")] // We want this test to panic since we are giving an unusable argument.
fn test_invalid_premature_withdrawal() {
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

    let token = TokenClient::new(&env, &id);

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

}

// This test, `test_valid_amount_updated` tests the `fix_amount` function.
// The contract is initialized with one amount and the user account
// makes an update to the recurring transfer amount. The test validates
// that the updated value is reflected in the transfer balance. 
#[test]
fn test_valid_amount_updated() {
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

    let token = TokenClient::new(&env, &id);

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

    client.with_source_account(&u1).init(
        &u2,         // our `receiver` account
        &id,         // our token contract id
        &1601129600, // Start date
        &10000000,
        &(7 * 24 * 60 * 60), // 1 withdraw per week
    );

    // Update the amount to something different
    client.with_source_account(&u2).fix_amount(&400000000);

    client.withdraw();
    // The amount transferred should reflect the update
    assert_eq!(token.balance(&Identifier::Account(u2.clone())), 400000000);

}

// This test, `test_invalid_withdraw_after_change_step`, updates the recurring
// payment cadence. After changing the step, the user attempts to withdraw
// without waiting the new time. The test should error with `ReceiverAlreadyWithdrawn`
#[test]
#[should_panic(expected = "Status(ContractError(4))")] 
fn test_invalid_withdraw_after_change_step() {

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

    let token = TokenClient::new(&env, &id);

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

    client.with_source_account(&u1).init(
        &u2,         // our `receiver` account
        &id,         // our token contract id
        &1669680000, // Past date
        &10000000,
        &(7 * 24 * 60 * 60), // 1 withdraw per week
    );

    client.withdraw();
    assert_eq!(token.balance(&Identifier::Account(u2.clone())), 10000000);

    // Update the step to only allow monthly withdraws
    client.with_source_account(&u1).fix_step(&(30 * 24 * 60 * 60));

    // Increment the ledger by a week. In the past scenario, 
    // this should have allowed the user to withdraw again!
    env.ledger().set(LedgerInfo {
        timestamp: 1670371200, // about 8 days in the future
        protocol_version: 1,
        sequence_number: 10,
        network_passphrase: Default::default(),
        base_reserve: 10,
    }); 

    client.withdraw();

}