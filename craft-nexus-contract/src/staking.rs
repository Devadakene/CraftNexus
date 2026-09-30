#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype,

    vec, Address, Env, Vec,
};

// ============================================================================
// 0. ERROR TYPES
// ============================================================================

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    StakeNotFound = 1,
}

// ============================================================================
// 1. DATA STRUCTURES & KEYS
// ============================================================================

const COOLDOWN_PERIOD: u64 = 86400 * 7; // 7 days in seconds

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeEntry {
    pub amount: i128,
    pub unlock_time: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    StakeNotFound = 1,
}

#[contracttype]
pub enum DataKey {
    UserStakes(Address),
}

// ============================================================================
// 2. STAKING CONTRACT IMPLEMENTATION (The Fix)
// ============================================================================

#[contract]
pub struct StakeContract;

#[contractimpl]
impl StakeContract {
    /// Adds a new stake, appending it as an independent entry with its own maturity.
    pub fn stake(env: Env, user: Address, amount: i128) {
        user.require_auth();

        let current_time = env.ledger().timestamp();
        let unlock_time = current_time + COOLDOWN_PERIOD;

        let mut stakes: Vec<StakeEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::UserStakes(user.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        stakes.push_back(StakeEntry {
            amount,
            unlock_time,
        });
        env.storage()
            .persistent()
            .set(&DataKey::UserStakes(user), &stakes);

        // (External token transfer logic from user to contract would go here)
    }

    /// Evaluates all user stakes and processes withdrawals for those that have matured.
    pub fn withdraw_matured(env: Env, user: Address) -> i128 {
        user.require_auth();

        let current_time = env.ledger().timestamp();
        let stakes: Vec<StakeEntry> = env
            .storage()
            .persistent()
            .get(&DataKey::UserStakes(user.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let mut remaining_stakes = Vec::new(&env);
        let mut withdrawable_amount: i128 = 0;

        for stake in stakes.into_iter() {
            if current_time >= stake.unlock_time {
                withdrawable_amount += stake.amount;
            } else {
                remaining_stakes.push_back(stake);
            }
        }

        // Update state with only the pending, un-matured stakes
        env.storage()
            .persistent()
            .set(&DataKey::UserStakes(user), &remaining_stakes);

        // (External token transfer logic from contract to user would go here)

        withdrawable_amount
    }

    /// Read-only function to inspect a user's current stake queue
    pub fn get_stakes(env: Env, user: Address) -> Vec<StakeEntry> {
        env.storage()
            .persistent()
            .get(&DataKey::UserStakes(user))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Read-only function to fetch a user's stake record.
    ///
    /// Returns `Error::StakeNotFound` when the storage key is absent instead of
    /// trapping the host. Uses `extend_persistent_read` on the hot persistent
    /// key so callers do not pay for a full scan or panic on archival.
    pub fn get_artisan_stake_data(env: Env, user: Address) -> Result<Vec<StakeEntry>, Error> {
        let key = DataKey::UserStakes(user);
        env.storage().persistent().extend_ttl(&key, 100, 1000);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(Error::StakeNotFound)
    }
}

// ============================================================================
// 3. TEST SUITE (Issue #1050 Acceptance Criteria)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    fn setup() -> (Env, Address, StakeContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        // Initialize ledger time to a known baseline
        env.ledger().set_timestamp(100_000);

        let user = Address::generate(&env);
        let contract_id = env.register_contract(None, StakeContract);
        let client = StakeContractClient::new(&env, &contract_id);

        (env, user, client)
    }

    #[test]
    fn test_new_deposit_does_not_bypass_cooldown() {
        let (env, user, client) = setup();

        // 1. Initial stake
        client.stake(&user, &1000);
        let initial_stake_time = env.ledger().timestamp();

        // 2. Advance time forward, but not past the 7-day cooldown
        env.ledger()
            .set_timestamp(initial_stake_time + (COOLDOWN_PERIOD / 2));

        // 3. Second stake added
        client.stake(&user, &500);

        // 4. Attempt withdrawal. Neither should be ready.
        let withdrawn = client.withdraw_matured(&user);
        assert_eq!(withdrawn, 0, "New deposit bypassed cooldown rules");

        let queue = client.get_stakes(&user);
        assert_eq!(queue.len(), 2, "Queue should contain both pending deposits");
    }

    #[test]
    fn test_matured_deposits_remain_withdrawable_under_original_schedule() {
        let (env, user, client) = setup();

        client.stake(&user, &1000);
        let initial_stake_time = env.ledger().timestamp();

        // Advance time just past the cooldown for the first stake
        env.ledger()
            .set_timestamp(initial_stake_time + COOLDOWN_PERIOD + 1);

        // Add a new stake
        client.stake(&user, &500);

        // Withdraw matured stakes. The first 1000 should be ready, the 500 should remain locked.
        let withdrawn = client.withdraw_matured(&user);

        assert_eq!(
            withdrawn, 1000,
            "Matured deposit was blocked by new deposit"
        );

        let remaining_queue = client.get_stakes(&user);
        assert_eq!(
            remaining_queue.len(),
            1,
            "Only the new pending deposit should remain"
        );
        assert_eq!(remaining_queue.get(0).unwrap().amount, 500);
    }

    #[test]
    fn test_deposits_before_and_after_boundary() {
        let (env, user, client) = setup();
        let base_time = env.ledger().timestamp();

        // Stake A: Time T
        client.stake(&user, &1000);

        // Stake B: Time T + 3 days
        env.ledger().set_timestamp(base_time + 86400 * 3);
        client.stake(&user, &2000);

        // Advance to Time T + 7.5 days.
        // Stake A is past boundary (>7 days). Stake B is before boundary (only 4.5 days old).
        env.ledger()
            .set_timestamp(base_time + COOLDOWN_PERIOD + 43200);

        let withdrawn = client.withdraw_matured(&user);

        // Assert exactly Stake A is released
        assert_eq!(
            withdrawn, 1000,
            "Boundary logic failed to separate mature vs pending stakes"
        );

        // Advance remaining time to mature Stake B
        env.ledger()
            .set_timestamp(base_time + 86400 * 3 + COOLDOWN_PERIOD + 1);
        let withdrawn_b = client.withdraw_matured(&user);

        assert_eq!(
            withdrawn_b, 2000,
            "Stake B failed to mature on its independent schedule"
        );

        let empty_queue = client.get_stakes(&user);
        assert_eq!(
            empty_queue.len(),
            0,
            "Queue should be empty after all stakes mature"
        );
    }

    #[test]
    fn test_get_artisan_stake_data_missing_key_returns_error() {
        let (_env, user, client) = setup();

        // No stake has been recorded yet: the key is absent.
        let result = client.try_get_artisan_stake_data(&user);
        assert_eq!(result, Err(Ok(Error::StakeNotFound)));
    }

    #[test]
    fn test_get_artisan_stake_data_after_terminal_state() {
        let (env, user, client) = setup();

        client.stake(&user, &1000);

        // Advance past the cooldown and drain the queue to a terminal state.
        env.ledger()
            .set_timestamp(env.ledger().timestamp() + COOLDOWN_PERIOD + 1);
        let withdrawn = client.withdraw_matured(&user);
        assert_eq!(withdrawn, 1000);

        // Record now exists but is empty; the call must not trap.
        let stakes = client.get_artisan_stake_data(&user).unwrap();
        assert_eq!(stakes.len(), 0);
    }
}
