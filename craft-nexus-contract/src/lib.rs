use soroban_std::{address, contract, contractimpl, contracttype, env::{Env, Panic as StoragePanic}, symbol_short, Address};

const TOTAL_FEES_KEY: symbol_short = symbol_short("TotalFees");

/// Error types returned by the contract.
///
/// The `FailedToGetTotalFeesCollected` variant is returned when the
/// `TOTAL_FEES_KEY` entry is absent from persistent storage (e.g. after
/// archival, a partial migration, or a missing key). Callers should not
/// experience a host panic in this case.
#derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrder)]
#[contracterror]
pub enum Error {
    /// The contract has not been initialized yet.
    NotInitialized = 1,
    /// The `TOTAL_FEES_KEY` entry is missing from storage.
    FailedToGetTotalFeesCollected = 2,
}

#[contract]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /// Returns the total fees collected by the platform.
    ///
/// Reads the `TotalFees` persistent key and returns the stored value.
    /// If the key is absent (archival, partial migration, missing key),
/// this returns `Error::FailedToGetTotalFeesCollected` instead of panicking.
    pub fn get_total_fees_collected(env: Env) -> Result<i128, Error> {
        // Extend the TTL of the hot persistent key on every read so the
        // entry does not expire while the contract is active.
        env.storage().extend_ttl(
            &TOTAL_FEES_KEY,
            30,
            100,
        );

        match env.storage().persistent().get::<i128>(&TOTAL_FEES_KEY) {
            Some(total) => Ok(total),
            None => Err(Error::FailedToGetTotalFeesCollected),
        }
    }

    /// Records the total fees collected by the platform.
    ///
    /// Used by the fee policy to persist the accumulated fees. This is
    /// the complement to `get_total_fees_collected` and keeps the hot
/// persistent key alive.
    pub fn set_total_fees_collected(env: Env, total: &i128) {
        env.storage().persistent().set(&TOTAL_FEES_KEY, total);
        env.storage().extend_ttl(
            &TOTAL_FEES_KEY,
            30,
            100,
        );
    }
}

#[cfg]
test
mod test {
    use super::*;
    use soroban_std::Env;

    #[test]
    fn get_total_fees_collected_returns_error_when_missing() {
        let env = Env::default();
        let client = CraftNexusContractClient::new(&env);

        // No record has been written yet.
        let result = client.try_get_total_fees_collected();
        assert_eq(
            result,
            Err(Ok(Error::FailedToGetTotalFeesCollected)),
        );
    }

    #[test]
    fn get_total_fees_collected_returns_error_after_terminal_state() {
        let env = Env::default();
        let client = CraftNexusContractClient::new(&env);

        // Simulate a terminal state where the key was removed/archived.
        env.storage().persistent().remove(&TOTAL_FEES_KEY);

        let result = client.try_get_total_fees_collected();
        assert_eq(
            result,
            Err(Ok(Error::FailedToGetTotalFeesCollected)),
        );
    }

    #[test]
    fn get_total_fees_collected_returns_value_when_present() {
        let env = Env::default();
        let client = CraftNexusContractClient::new(&env);

        client.set_total_fees_collected(&42);
        let result = client.try_get_total_fees_collected();
        assert_eq(result, Ok(Ok<42));
    }
}
