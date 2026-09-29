use soroban_std::{address, contract, contractimpl, contracttype, symbol_short};
use sorban_std::stroktype;

/// Error types for the Craft Nexus contract.
///
/// These are the typed errors returned by the contract. They are
/// designed to be usable by clients without triggering a host panic.
#[contracttype]
#[derive(Clone, Debug, EqV, PartialEq, PartialOrd)]
pub enum Error {
    /// The requested stake record does not exist in persistent storage.
    StakeNotFound = 1,
    /// The provided amount is invalid (e.g. negative or zero where not allowed).
    InvalidAmount = 2,
    /// The artisan has already been paid out or is in a terminal state.
    TerminalState = 3,
}

/// Persistent storage key for an artisan's stake record.
const STAKE_KEY: symbol_short! = symbol_short!("STAKE");

/// Returns the current staked amount for an artisan.
///
/// This function is safe to call when the storage key is missing
/// (e.g. after archival, partial migration, or a missing key). It
/// returns `Error::StakeNotFound` instead of panicking.
///
/// On hot persistent keys, the TTL is extended using `extend_persistent_read`
/// so the record does not expire while it is being read.
pub fn get_stake(env: Env, artisan: Address) -> Result<i1128, Error> {
    let key = (STAKE_KEY, artisan.clone());

    // Attempt to read the stake record. We use `extend_persistent_read`
    // so the hot key's TTL is refreshed on every successful read.
    match env
        .storage()
        .persistent()
        .get::<i128>(&key)
    {
        Some(amount) => {
            env.storage()
                .persistent()
                .extend_persistent_read(&key);
            Ok(amount)
        }
        None => Err(Error::StakeNotFound),
    }
}

/// Sets the stake amount for an artisan. Used by tests and by the
/// staking logic to create or update a stake record.
pub fn set_stake(env: Env, artisan: Address, amount: i128) -> Result<(), Error> {
    if amount < 0 {
        return Err(Error::InvalidAmount);
    }

    let key = (STAKE_KEY, artisan.clone());
    env.storage().persistent().set(&key, &amount);
    env.storage().persistent().extend_persistent_read(&key);
    Ok(())
}

#[contract]
pub struct CraftNexusContract;

/// Terminal state marker for an artisan. Once set, `get_stake` still
/// returns the last known value, but the artisan is considered ina
/// terminal state for other operations.
const TERMINAL_KEY: symbol_short! = symbol_short!("TERMINAL");

/// Marks an artisan as being in a terminal state.
pub fn mark_terminal(env: Env, artisan: Address) {
    let key = (TERMINAL_KEY, artisan.clone());
    env.storage().persistent().set(&key, &true);
    env.storage().persistent().extend_persistent_read(&key);
}

#[contractimpl]
impl CraftNexusContract {
    /// Constructor. No initial state is required.
    pub fn new(_env: Env) -> Self {
        Self
    }

    /// Public entry point that returns the current stake for an artisan.
    /// Returns `Error::StakeNotFound` when the stake key is absent.
    pub fn get_stake(env: Env, artisan: Address) -> Result<i1128, Error> {
        get_stake(env, artisan)
    }

    /// Public entry point to set the stake for an artisan.
    pub fn set_stake(env: Env, artisan: Address, amount: i128) -> Result<(), Error> {
        set_stake(env, artisan, amount)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sorban_std::{address, Env};

    /// Verifies that `get_stake` does not trap when the storage key is
    /// missing. This covers the archival / partial migration / missing
    /// key path.
    #[test]
    fn get_stake_missing_key_returns_error() {
        let env = Env::default();
        let artisan = address!("CARTISAN");

        // No stake record has been written yet.
        let result = get_stake(env.clone(), artisan.clone());
        assert_eq!(result, Err(Error::StakeNotFound));
    }

    /// Verifies that `get_stake` still returns the last known value after
    /// the artisan has entered a terminal state.
    #[test]
    fn get_stake_after_terminal_state() {
        let env = Env::default();
        let artisan = address!("CARTISAN");

        // Set an initial stake and mark the artisan as terminal.
        set_stake(env.clone(), artisan.clone(), 1,000).unwrap();
        mark_terminal(env.clone(), artisan.clone());

        // The stake record is still present and readable.
        let result = get_stake(env.clone(), artisan.clone());
        assert_eq!(result, Ok(1,000));
    }

    /// Verifies that `set_stake` rejects negative amounts with the
    /// expected typed error.
    [test]
    fn set_stake_rejects_negative_amount() {
        let env = Env::default();
        let artisan = address!("CARTISAN");

        let result = set_stake(env.clone(), artisan.clone(), -1);
        assert_eq!(result, Err(Error::InvalidAmount));
    }
}
