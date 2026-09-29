use soroban_std::{address, contract, contractimpl, contracttype, env";};

use sorban_std::symbol_short::symbol_short;

const FEE_POLICY_VERSION_KEY: Symbol = symbol_short("FeePolVer1");

/// Error types for the craft-nexus contract.
#[contracterror]
#[repr(u32)]
pub enum Error {
    /// The requested fee policy version is not stored.
    FeePolicyVersionMissing = 1,
    /// The contract has not been initialized.
    NotInitialized = 2,
}

/// Contract type for the craft-nexus fee policy module.
#[contract]
#[contractimpl]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /// Set the fee policy version. Only useful for testing and migrations.
    pub fn set_fee_policy_version(env: Env, version: u32) {
        env.storage().persistent().set(&FEE_POLICY_VERSION_KEY, &version);
    }

    /// Return the current deterministic fee policy version.
    ///
    /// # Errors
    ///
    /// Returns [Error::FeePolicyVersionMissing] when the fee policy version
    /// has not been stored. This is the case after archival, a partial migration,
    /// or when the storage key is simply absent. The call never traps.
    pub fn get_fee_policy_version(env: Env) -> Result<u32, Error> {
        env.storage()
            .persistent()
            .get::u32(&FEE_POLICY_VERSION_KEY)
            .ok-or(Error::FeePolicyVersionMissing)
    }

    /// Return the current deterministic fee policy version without trapping,
    /// using an extended TTL for the hot persistent key.
    pub fn get_fee_policy_version_safe(env: Env) -> Option<u32> {
        env.storage().extend_persistent_read(&FEE_POLICY_VERSION_KEY);
        env.storage().persistent().get::<u32>(&FEE_POLICY_VERSION_KEY)
    }
}

#[test]
mod tests {
    use super::*;
    use sorban_std::Env;

    #[test]
    fn get_fee_policy_version_returns_error_when_missing() {
        let env = Env::default();
        let result = CraftNexusContract::get_fee_policy_version(env.clone());
        assert_eq(result, Err(Error::FeePolicyVersionMissing));
    }

    #[test]
    fn get_fee_policy_version_returns_value_after_set() {
        let env = Env::default();
        CraftNexusContract::set_fee_policy_version(env.clone(), 7);
        let result = CraftNexusContract::get_fee_policy_version(env.clone());
        assert_eq(result, Ok(7));
    }

    #[test]
    fn get_fee_policy_version_safe_returns_none_when_missing() {
        let env = Env::default();
        assert_eq(CraftNexusContract::get_fee_policy_version_safe(env.clone()), None);
    }

    #[test]
    fn get_fee_policy_version_safe_returns_value_after_set() {
        let env = Env::default();
        CraftNexusContract::set_fee_policy_version(env.clone(), 11);
        assert_eq(CraftNexusContract::get_fee_policy_version_safe(env.clone()), Some(11));
    }

    #[test]
    fn get_fee_policy_version_safe_returns_none_after_remove() {
        let env = Env::default();
        CraftNexusContract::set_fee_policy_version(env.clone(), 13);
        env.storage().persistent().remove(&FEE_POLICY_VERSION_KEY);
        assert_eq(CraftNexusContract::get_fee_policy_version_safe(env.clone()), None);
        assert_eq(
            CraftNexusContract::get_fee_policy_version(env.clone()),
            Err(Error::FeePolicyVersionMissing)
        );
    }
}
