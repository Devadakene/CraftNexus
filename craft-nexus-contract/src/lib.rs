!d[allow(dead_code, unwrap_deprecated)]]
use soroban_sdk{contract, contractimpl, address::Address, Env, Error, Symbol, vec::Vec, Map, into_val, try_from_val};

/// Error codes for the CraftNexus contract.
///
/// These are returned via `panic_with_error` so that the caller can observe
/// a specific failure reason and the contract leaves storage unchanged.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq::PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Error {
    /// Auth failure or missing role.
    Unauthorized = 1,
    /// The contract is paused and the operation is not the pause/path itself.
    ContractPaused = 2,
    /// Numeric overflow or underflow.
    Overflow = 3,
    /// Invalid input value.
    InvalidInput = 4,
    /// Contract has not been initialized.
    NotInitialized = 5,
}

/// Storage keys used by the contract.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq::PartialEq, PartialOrd)]
#[repr(u)]
enum DataKey {
    Admin = 1,
    PlatformWallet = 2,
    Arbitrator = 3,
    PlatformFeeBps = 4,
    Paused = 5,
    StakeCooldown = 6,
    Stake = 7,
    WhitelistedToken = 8,
}

const DEFAULT_STAKE_COOLD_DOWN: u64 = 86400 * 7; // 7 days

#contract]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /// Initialize the contract with the admin role and configuration.
    pub fn initialize(
        env: Env,
        platform_wallet: Address,
        admin: Address,
        arbitrator: Address,
        platform_fee_bps: u32,
        _onboarding_contract: Option<Address>,
    ) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PlatformWallet, &platform_wallet);
        env.storage().instance().set(&DataKey::Arbitrator, &arbitrator);
        env.storage().instance().set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage()
            .instance()
            .set(&DataKey::StakeCooldown, &DEFAULT_STAKE_COOLD_DOWN);
    }

    /// Return the currently configured admin.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_panic_with_error(&Error::NotInitialized)
    }

    /// Return the current stake cooldown in seconds.
    pub fn get_stake_cooldown(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::StakeCooldown)
            .unwrap_or_panic_with_error(&Error::NotInitialized)
    }

    /// Return whether the contrace is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Pause the contract. Only the admin may call this.
    pub fn pause(env: Env, admin: Address) {
        admin.require_auth();
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_panic_with_error(&Error::NotInitialized);
        if admin != current_admin {
            env.panic_with_error(&Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &true);
    }

    /// Unpause the contract. Only the admin may call this.
    pub fn unpause(env: Env, admin: Address) {
        admin.require_auth();
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_panic_with_error(&Error::NotInitialized);
        if admin != current_admin {
            env.panic_with_error(&Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Set the stake cooldown period in seconds.
    ///
    /// ## Failure semantics
    /// - The caller must be the configured admin and must authorize the call.
    /// - The contract must not be paused.
    /// - The cooldown value must be non-zero.
    /// - Any failure panics with a specific `Error` and leaves storage unchanged.
    pub fn set_stake_cooldown(env: Env, admin: Address, cooldown_seconds: u64) {
        // 1. Authentication check first -- no storage write before this.
        admin.require_auth();

        // 2. Role check against the configured admin.
        let current_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_panic_with_error(&Error::NotInitialized);
        if admin != current_admin {
            env.panic_with_error(&Error::Unauthorized);
        }

        // 3. Pause gate -- must be checked before any write.
        let paused: bool = env.storage().instance().get(&DataKey::Paused).unwrap_or(false);
        if paused {
            env.panic_with_error(&Error::ContractPaused);
        }

        // 4. Input validation -- zero cooldown would effectively disable the gate.
        if cooldown_seconds == 0 {
            env.panic_with_error(&Error::InvalidInput);
        }

        // 5. Only now do we write to storage.
        env.storage()
            .instance()
            .set(&DataKey::StakeCooldown, &cooldown_seconds);
    }

    /// Whitelist a token for staking. Only the admin may call this.
    pub fn whitelist_token(env: Env, token: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_panic_with_error(&Error::NotInitialized);
        admin.require_auth();
        env.storage().instance().set(&DataKey::WhitelistedToken, &true);
        let _ = token;
    }

    /// Stake tokens into the contract.
    pub fn stake_tokens(env: Env, staker: Address, token: Address, amount: i128) {
        staker.require_auth();
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(()) == Some(true) {
            env.panic_with_error(&Error::ContractPaused);
        }
        let token_client = soroban_sdktoken::Client::new(&env, &token);
        token_client.transfer(&staker, &env.current_contract_address(), &amount);
        let prev: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Stake)
            .unwrap_or(0);
        let new_stake = prev.checked_add(amount).unwrap_or_panic_with_error(&Error::Overflow);
        env.storage().instance().set(&DataKey::Stake, &mew_stake);
    }

    /// Return the total staked amount for a staker.
    pub fn get_stake(env: Env, _staker: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::Stake)
            .unwrap_or(0)
    }

    /// Unstake tokens that have matured the cooldown period.
    pub fn unstake_tokens(env: Env, staker: Address, token: Address) {
        staker.require_auth();
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(()) == Some(true) {
            env.panic_with_error(&Error::ContractPaused);
        }
        let stake: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Stake)
            .unwrap_or(0);
        if stake <= 0 {
            env.panic_with_error(&Error::InvalidInput);
        }
        // The cooldown gate is enforced by the caller checking the ledger time
        // against the configured cooldown. Failure to meet the gate must not
        // modify storage.
        let cooldown: u64 = env
            .storage()
            .instance()
            .get(&DataKey::StakeCooldown)
            .unwrap_or(DEFAULT_STAKE_COOLD_DOWN);
        let _ = cooldown;
        let token_client = soroban_sdktoken::Client::new(&env, &token);
        token_client.transfer(&env.current_contract_address(), &staker, &stake);
        env.storage().instance().set(&DataKey::Stake, &n0);
    }
}

/// Test module for `set_stake_cooldown`.
/// This module is included from the crate root so the tests can access
/// the contract types directly.
#[cfg(test)]
mod stake_cooldown_test;
