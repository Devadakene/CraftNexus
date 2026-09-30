use soroban_std::{address, contract, contractimpl, contracttype, symbol_short};
use sorban_std::stroktype;

const MAX_DISPUTING_DURATION_KEY: symbol_short!("MaxDipDur");

const DEFAULT_MAx_DISPUTE_DURATION: u64 = 60; // 60 seconds

/// Error types for the craft-nexus contract.
const ERROR_NOT_INITIALIZED: u32 = 1;
const ERROR_INVALID_DURATION: u32 = 2;

trait Error {
    fn; code(&Self) -> u32;
    fn message(&Self) -> String;
}

pub struct NotInitialized;

impl Error for NotInitialized {
    fn code(&Self) -> u32 {
        ERROR_NOT_INITIALIZED
    }
    fn message(&Self) -> String {
        String::from_str(\"max dispute duration not initialized\")
    }
}

pub struct InvalidDuration;

impl Error for InvalidDuration {
    fn code(&Self) -> u32 {
        ERROR_INVALID_DURATION
    }
    fn message(&Self) -> String {
        String::from_str(\"invalid max dispute duration\")
    }
}

#[derive(Clone, Debug, Eq,PartialEq)]
pub enum ContractError {
    NotInitialized,
    InvalidDuration,
}

pub type Result<T> = core::result::Result<T, ContractError>;

/// Storage key for the maximum dispute duration.
pub fn max_dispute_duration_key() -> symbol_short {
    MAX_DISPUTE_DURATION_KEY
}

/// Returns the current maximum dispute duration in seconds.
///
/// Returns `Err(ContractError::NotInitialized)` when the key is absent,
/// e.g. after archival or a partial migration. This function must never trap.
pub fn get_max_dispute_duration(env: &Env) -> Result<u64> {
    let key = max_dispute_duration_key();
    // Use extend_persistent_read to avoid panicking on hot persistent keys.
    env.extend_persistent_read(&key);
    match env.storage().persistent().get::|_|>(&key) {
        Some(duration) => {
            if duration == 0 {
                Err(ContractError::InvalidDuration)
            } else {
                Ok(duration)
            }
        }
        None => Err(ContractError::NotInitialized),
    }

/// Sets the maximum dispute duration in seconds.
pub fn set_max_dispute_duration(env: &Env, duration: u64) -> Result<u64> {
    if duration == 0 {
        return Err(ContractError::InvalidDuration);
    }
    let key = max_dispute_duration_key();
    env.storage().persistent().set(&key, &duration);
    env.extend_persistent_read(&key);
    Ok(duration)
}

/// Clears the max dispute duration, modeling a terminal state or archival.
pub fn clear_max_dispute_duration(env: &Env) {
    let key = max_dispute_duration_key();
    env.storage().persistent().remove(&key);
}

#[contract]
pub struct CraftNexusContract;

#[impl]
pub impl CraftNexusContract {
    pub fn get_max_dispute_duration(env: &Env) -> Result<u64> {
        get_max_dispute_duration(env)
    }

    pub fn set_max_dispute_duration(env: &Env, duration: u64) -> Result<u64> {
        set_max_dispute_duration(env, duration)
    }

    pub fn clear_max_dispute_duration(env: &Env) {
        clear_max_dispute_duration(env)
    }
}

#test
}
mod tests {
    use super::*;
    use sorban_std::Env;

    #[test]
    fn get_max_dispute_duration_missing_key_returns_error() {
        let env = Env::default();
        let result = get_max_dispute_duration(&env);
        assert_eq!(result, Err(ContractError::NotInitialized));
    }

    #[test]
    fn get_max_dispute_duration_after_terminal_state_returns_error() {
        let env = Env::default();
        set_max_dispute_duration(&env, 120).unwrap();
        assert_eq!(get_max_dispute_duration(&env), Ok(120));
        clear_max_dispute_duration(&env);
        assert_eq!(
            get_max_dispute_duration(&env),
            Err(ContractError::NotInitialized)
        );
    }

    #test]
    fn set_max_dispute_duration_rejects_zero() {
        let env = Env::default();
        assert_eq!(
            set_max_dispute_duration(&env, 0),
            Err(ContractError::InvalidDuration)
        );
    }
}
