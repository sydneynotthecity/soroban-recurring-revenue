#![no_std]

/// We're using the `soroban_auth` crate today to verify and authenticate users
/// and some invocations in our contract. It's a really powerful SDK to get
/// familiar with. https://soroban.stellar.org/docs/sdks/rust-auth
use soroban_auth::{Identifier, Signature};
use soroban_sdk::{contracterror, contractimpl, contracttype, AccountId, Address, BytesN, Env};
use soroban_token_spec::{TokenClient};

/// The `contractimport` macro will bring in the contents of the built-in
/// soroban token contract and generate a module we can use with it.
// mod token {
//     // soroban_sdk::contractimport!(file = "/soroban_token_spec.wasm");
//     soroban_sdk::contractimport!(file="./soroban_token_spec.wasm");
// }

/// An `Error` enum is used to meaningfully and concisely share error
/// information with a contract user.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    ContractAlreadyInitialized = 1,
    ContractNotInitialized = 2,
    InvalidAuth = 3,
    ReceiverAlreadyWithdrawn = 4,
    PrematureFirstWithdraw = 5,
    InvalidInvoker = 6,
    InvalidArguments = 7,
    ContractNotUpdated = 8,
}

/// We are using a `StorageKey` enum to store different types of data, but keying
/// those pieces of data in a centralized place. This aids in manageability and
/// makes it easier to adapt our contract to store additional pieces of data.
#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    Sender,  // AccountId
    Receiver,   // AccountId
    TokenId, // BytesN<32>
    StartEpoch, // u64
    Amount,  // i128
    Step,    // u64
    Latest,  // u64
}

pub struct RecurringRevenueContract;

pub trait RecurringRevenueTrait {
    // When `init`ializing the contract, we must specify some of the data that
    // will be stored (remember the `StorageKey`?) for the contract to reference.
    // We are using an `AccountId` for the `receiver` to highlight that a transfer
    // from one user to another is the intended use-case of this particular
    // contract. It also makes the Soroban CLI usage a bit cleaner and easier.
    fn init(
        e: Env,
        receiver: AccountId,     // the account receiving the recurring payment
        token_id: BytesN<32>, // the id of the token being transferred as a payment
        start_epoch: u64,     // the starting time (in UTC seconds) when the first payment begins
        amount: i128,         // the amount paid for each recurring payment
        step: u64,            // how frequently (in seconds) a withdrawal can be made
    ) -> Result<(), Error>;

    // When `withdraw` is invoked, a transfer is made from the `Sender` asset
    // balance to the `Receiver` asset balance. No signature required!
    fn withdraw(e: Env) -> Result<(), Error>;

    // When `fix_amount` is invoked, the amount that sent in a payment
    // is updated. The current amount cannot be the new amount.
    fn fix_amount(
        e: Env,
        amount: i128,          //the updated amount changed to the recurring payment
    ) -> Result<(), Error>;
}

/// When a contract uses "Invoker" authentication, `env.invoker()` returns the
/// `Address` type. Since we're storing an `AccountId` as the `Sender`, we use
/// a helper function to convert from one to the other.
fn to_account(address: Address) -> Result<AccountId, Error> {
    match address {
        Address::Account(id) => Ok(id),
        _ => Err(Error::InvalidInvoker),
    }
}

#[contractimpl]
impl RecurringRevenueTrait for RecurringRevenueContract {
    // Remember, before you can invoke `withdraw`, you must invoke `init`
    fn init(
        e: Env,
        receiver: AccountId,
        token_id: BytesN<32>,
        start_epoch: u64,
        amount: i128,
        step: u64,
    ) -> Result<(), Error> {
        // When running `init`, we want to make sure the function hasn't already
        // been invoked. Although a few different `StorageKey`s are set during
        // init, it's enough to only check for one.
        let token_key = StorageKey::TokenId;
        if e.storage().has(&token_key) {
            return Err(Error::ContractAlreadyInitialized);
        }

        // You can't have a withdraw every 0 seconds. Obviously. Also, you can't
        // divide by 0. So say the calculators, at least.
        if step == 0 {
            return Err(Error::InvalidArguments);
        }

        // A withdrawal should never be `0`. I mean, really. At that point, why
        // even go through the trouble of setting this up?
        if (amount * step as i128) == 0 {
            return Err(Error::InvalidArguments);
        }

        // We are setting up all the data that this contract will store on the
        // ledger here. Nothing fancy here, just the same thing a few times.
        e.storage().set(&token_key, &token_id);
        e.storage()
            .set(&StorageKey::Sender, &to_account(e.invoker()).unwrap()); // the invoker of `init` becomes the `Sender`
        e.storage().set(&StorageKey::Receiver, &receiver);
        e.storage().set(&StorageKey::StartEpoch, &start_epoch);
        e.storage().set(&StorageKey::Amount, &amount);
        e.storage().set(&StorageKey::Step, &step);

        // During contract init() the latest withdraw will be set as a time before the payment start time
        e.storage().set(&StorageKey::Latest, &(start_epoch - step));

        Ok(())
    }

    fn withdraw(e: Env) -> Result<(), Error> {
        // Conversely from `init`, we want to make sure the contract *has* been
        // initialized before a withdraw can be made.
        let key = StorageKey::TokenId;
        if !e.storage().has(&key) {
            return Err(Error::ContractNotInitialized);
        }

        // We create a client to the token contract that we'll be able to use to
        // make the transfer later on.
        let token_id: BytesN<32> = e.storage().get(&key).unwrap().unwrap();
        let client = token::Client::new(&e, &token_id);

        // This is a simple check to ensure the `withdraw` function has not been
        // invoked by a contract. For our purposes, it *must* be invoked by a
        // user account.
        match e.invoker() {
            Address::Account(id) => id,
            _ => return Err(Error::InvalidInvoker),
        };

        // This part is one of the contract's really nifty tricks. You may have
        // noticed we haven't authenticated the invocation of `withdraw` at all.
        // That's on purpose! By storing the `Receiver` in our contract data, we
        // can ensure they are *always* the beneficiary of the withdrawal. No
        // matter who actually makes the call to the contract, the receiver
        // always receives the funds payment.
        let receiver = e.storage().get(&StorageKey::Receiver).unwrap().unwrap();
        // Note: Technically speaking, *anybody* could invoke the `withdraw`
        // function in the contract.

        let step: u64 = e.storage().get(&StorageKey::Step).unwrap().unwrap();
        let amount: i128 = e.storage().get(&StorageKey::Amount).unwrap().unwrap();

        // Check that the Receiver is allowed to start receiving payments
        let start_epoch: u64 = e.storage().get(&StorageKey::StartEpoch).unwrap().unwrap();
        if start_epoch > e.ledger().timestamp(){
            return Err(Error::PrematureFirstWithdraw)
        }

        // Some more quick math to make sure the `Latest` withdraw occurred *at
        // least* `step` seconds ago. 
        let latest: u64 = e.storage().get(&StorageKey::Latest).unwrap().unwrap();
        if latest + step > e.ledger().timestamp() {
            return Err(Error::ReceiverAlreadyWithdrawn);
        }

        // This is where the magic happens! We use the client we set up for our
        // token contract earlier to invoke the `xfer_from` function. We're
        // using *this contract's* approval to spend the asset balance of the
        // `Sender` account to transfer funds *directly* from the `Sender` to
        // the `Receiver`. That's amazing! Think of the implications and
        // possibilities! They're (and I mean this quite literally) endless!
        client.xfer_from(
            &Signature::Invoker,
            &(0 as i128),
            &Identifier::Account(e.storage().get(&StorageKey::Sender).unwrap().unwrap()),
            &Identifier::Account(receiver),
            &amount,
        );

        // We quickly set a new `Latest` in our contract data to reflect that
        // another withdraw has taken place. The astute among you may notice
        // this isn't based off the ledger's `timestamp()`, but rather the
        // latest withdraw. This allows the receiver to "catch up" on any missed
        // withdrawals. 
        let new_latest = latest + step;
        e.storage().set(&StorageKey::Latest, &new_latest);

        Ok(())
    }

    fn fix_amount(
        e: Env,
        amount: i128,
    ) -> Result<(), Error> {

        if amount == 0 {
            return Err(Error::InvalidArguments)
        } 

        // Confirm that that contract already exists. You
        // cannot modify a contract that does not exist.
        let token_key = StorageKey::TokenId;
        if !e.storage().has(&token_key) {
            return Err(Error::ContractNotInitialized);
        }

        // Check that the new amount does not match the current set amount.
        let old_amount: i128 = e.storage().get(&StorageKey::Amount).unwrap().unwrap();
        if old_amount == amount {
            return Err(Error::InvalidArguments)
        }

        // Set the Storage key amount to the new amount, fetch the amount to 
        // check that the contract actually updated.
        e.storage().set(&StorageKey::Amount, &amount);
        let updated_amount: i128 = e.storage().get(&StorageKey::Amount).unwrap().unwrap();
        if updated_amount!=amount {
            return Err(Error::ContractNotUpdated)
        }

        Ok(())

    }
}

mod test;
