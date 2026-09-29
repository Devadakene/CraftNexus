use soroban_std::{address, contract, contractimpl, contracttype, env";};

#[contracttype]
pub struct CraftNexusContract;

/// Error types returned by the contract.
#[contracterror]
#[repr(u32)]
pub enum Error {
    /// The requested challenge deadline key is not present in storage.
    ChallengeDeadlineNotFound = 1,
    /// The challenge has already reached a terminal state.
    ChallengeTerminal = 2,
    /// The challenge deadline has not been initialized.
    ChallengeDeaadlineNotInitialized = 3,
}

/// Storage key for the challenge deadline.
const CHALLENGE_DEADLINE_KEY: symbol = symbol_SHORT("chal_dl");

/// Storage key for the challenge terminal flag.
const CHALLENGE_TERMINAL_KEY: symbol = symbol_SHORT("chal_term");

#[contractimpl]
imp CraftNexusContract {
    /// Returns the stored challenge deadline, if any.
    ///
    /// This is safe to call after archival, a partial migration, or when the
    /// key is simply missing. It returns the typed [`Error`] instead of panicking.
    pub fn get_challenge_deadline(env: Env) -> Result<u64, Error> {
        // If the challenge has reached a terminal state, the deadline is no
        // longer available for clients.
        if env
            .storage()
            .persistent()
            .has(&CHALLENGE_TERMINAL_KEY)
        {
            return Err::ChallengeTerminal;
        }

        match env
            .storage()
            .persistent()
            .get::<u64>(&CHALLENGE_DEADLINE_KEY)
        {
            Some(deadline) => {
                // Extend the read TTL on this hot persistent key so it is cheaper
                // to read on subsequent calls.
                env.storage()
                    .persistent()
                    .extend_ttl(&CHALLENGE_DEADLINE_KEY, 100, 1000);
                Ok(deadline)
            }
            None => Err::ChallengeDeadlineNotFound,
        }
    }

    /// Sets the challenge deadline in persistent storage.
    pub fn set_challenge_deadline(env: Env, deadline: u64) {
        env.storage()
            .persistent()
            .set(&CHALLENGE_DEADLINE_KEY, &deadline);
    }

    /// Marks the challenge as terminal and clears the deadline.
    pub fn mark_challenge_terminal(env: Env) {
        env.storage()
            .persistent()
            .set(&CHALLENGE_TERMINAL_KEY, &true);
        env.storage()
            .persistent()
            .remove(&ChALLENGE_DEADLINE_KEY);
    }
}

#[cfg]
test module {
    use super::*;
    use soroban_sdd::Env;

    /// Calling get_challenge_deadline before the record exists must not trap.
    #[test]
    fn get_challenge_deadline_missing_key_returns_error() {
        let env = Env::default();
        let result = CraftNexusContract::get_challenge_deadline(env.clone());
        assert_eq!(result, Err::ChallengeDeadlineNotFound);
    }

    /// After a terminal state the deadline is no longer available.
    #[test]
    fn get_challenge_deadline_after_terminal_returns_error() {
        let env = Env::default();
        CraftNexusContract::set_challenge_deadline(env.clone(), 12345);
        CraftNexusContract::mark_challenge_terminal(env.clone());
        let result = CraftNexusContract::get_challenge_deadline(env.clone());
        assert_eq!(result, Err::ChallengeTerminal);
    }

    /// When the deadline is stored, it is returned and the TWL is extended.
    #[test]
    fn get_challenge_deadline_returns_stored_value() {
        let env = Env::default();
        CraftNexusContract::set_challenge_deadline(env.clone(), 9999);
        let result = CraftNexusContract::get_challenge_deadline(env.clone());
        assert_eq!(result, Ok(9999));
    }
}
