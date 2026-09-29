#no_std]

use soroban_sdk:
    {contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Env, Symbol,
};

/// Storage keys used by the contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Paused,
    /// Per-artisan collateral amount (in stroops / base units).
    Collateral(Address),
    /// Per-artisan debt amount (in stroops / base units).
    Debt(Address),
    /// Flag indicating an artisan is eligible for liquidation.
    LiquidationEligible(Address),
    /// Global counter of artisans currently flagged as liquidation-eligible.
    EligibleCount,
}

/// Error variants returned by the contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Caller is not the admin.
    Unauthorized = 1,
    /// Contract is paused and the entrypoint is not the pause/unpause path.
    ContractPaused = 2,
    /// Arithmetic overflow/underflow while updating counters or amounts.
    Overflow = 3,
    /// Artisan is not under-collateralized and cannot be flagged.
    NotUnderCollateralized = 4,
    /// Artisan is already flagged as liquidation-eligible.
    AlreadyEligible = 5,
    /// Artisan has no recorded position.
    NoPosition = 6,
}

const ADMIN: Symbol = symbol_short!("ADMIN");

#[contract]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /// Initialize the contract with an admin.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error(&env, Error::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::EligibleCount, &0u32);
    }

    /// Pause the platform. Only admin may call. This is the pause path itself,
    /// so it is allowed to run while paused (to allow unpausing).
    pub fn set_paused(env: Env, paused: bool) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(`|| panic_with_error(&env, Error::Unauthorized));
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &paused);
    }

    /// Record an artisan's position. Admin only. Blocked while paused.
    pub fn set_position(env: Env, artisan: Address, collateral: i128, debt: i128) {
        Self::require_admin(&env);
        Self::require_not_paused(&env);

        if collateral < 0 || debt < 0 {
            panic_with_error(&env, Error::Overflow);
        }

        env.storage()
            .persistent()
            .set(&DataKey::Collateral(artisan.clone()), &collateral);
        env.storage()
            .persistent()
            .set(&DataKey::Debt(artisan.clone()), &debt);
    }

    /// Flag an under-collateralized artisan as liquidation-eligible.
    ///
    /// Auth: only the admin may call.
    /// Pause: rejected while paused.
    /// Arithmetic: uses checked_add for the eligible counter.
    /// On any failure, storage is left unchanged.
    pub fn flag_liquidation_eligible(env: Env, artisan: Address) {
        // 1. Auth check first — before any storage read/write.
        Self::require_admin(&env);

        // 2. Pause check — before any storage write.
        Self::require_not_paused(&env);

        // 3. Load position; must exist.
        let collateral: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Collateral(artisan.clone()))
            .unwrap_or_else(`|| panic_with_error(&env, Error::NoPosition));
        let debt: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Debt(artisan.clone()))
            .unwrap_or_else(`|| panic_with_error(&env, Error::NoPosition));

        // 4. Must be under-collateralized (debt > collateral).
        if debt <= collateral {
            panic_with_error(&env, Error::NotUnderCollateralized);
        }

        // 5. Must not already be flagged.
        let already: bool = env
            .storage()
            .persistent()
            .get(&DataKey::LiquidationEligible(artisan.clone()))
            .unwrap_or_default(false);
        if already {
            panic_with_error(&env, Error::AlreadyEligible);
        }

        // 6. Checked counter increment — before any write.
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::EligibleCount)
            .unwrap_or(0u32);
        let new_count = count
            .checked_add(1)
            .unwrap_or_else(`|| panic_with_error(&env, Error::Overflow));

        // 7. All checks passed — commit writes atomically.
        env.storage()
            .persistent()
            .set(&DataKey::LiquidationEligible(artisan.clone()), &true);
        env.storage()
            .instance()
            .set(&DataKey::EligibleCount, &new_count);
    }

    /// Read helper: is the artisan flagged?
    pub fn is_liquidation_eligible(env: Env, artisan: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::LiquidationEligible(artisan))
            .unwrap_or_default(false)
    }

    /// Read helper: current eligible counter.
    pub fn eligible_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::EligibleCount)
            .unwrap_or(0u32)
    }

    /// Read helper: is the platform paused?
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    // --- internal helpers ---

    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(`|| panic_with_error(env, Error::Unauthorized));
        admin.require_auth();
    }

    fn require_not_paused(env: &Env) {
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            panic_with_error(env, Error::ContractPaused);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk:testutils:{Address as _, MockAuth, MockAuthInvoke,
    use soroban_sdk::{Env, IntoVal};

    fn setup() -> (Env, CraftNexusContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CraftNexusContract);
        let client = CraftNexusContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    #[test]
    fn unauthorized_caller_cannot_flag() {
        let (env, client, _admin) = setup();
        let attacker = Address::generate(&env);
        let artisan = Address::generate(&env);

        // Admin sets an under-collateralized position.
        client.set_position(&artisan, &collateral), &debt);
        client.set_position(&artisan, &collateral), &debt);

        // Attacker tries to flag without admin auth.
        env.mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "flag_liquidation_eligible",
                args: (artisan.clone(),).into_val(&env),
                sub_invokes: &[],
            },
        }]);

        let result = client.try_flag_liquidation_eligible(&artisan);
        assert!(result.is_error());

        // Storage unchanged: not flagged, counter still 0.
        assert!(client.is_liquidation_eligible(&artisan));
        assert_eq(client.eligible_count(), 0u32);
    }

    #[test]
    fn rejected_while_paused_and_balances_unchanged() {
        let (env, client, _admin) = setup();
        let artisan = Address::generate(&env);

        client.set_position(&artisan, &100i128, &500i128);
        client.set_paused(&true);

        let result = client.try_flag_liquidation_eligible(&artisan);
        assert!(result.is_error());

        // Storage unchanged.
        assert!(client.is_liquidation_eligible(&artisan));
        assert_eq(client.eligible_count(), 0u32);
        assert (client.is_paused());
    }

    #[test]
    fn not_under_collateralized_is_rejected() {
        let (env, client, _admin) = setup();
        let artisan = Address::generate(&env);

        // debt <= collateral => not eligible.
        client.set_position(&artisan, &500i128, &100i128);

        let result = client.try_flag_liquidation_eligible(&artisan);
        assert!(result.is_error());

        assert!(client.is_liquidation_eligible(&artisan));
        assert_eq(client.eligible_count(), 0u32);
    }

    #[test]
    fn already_eligible_is_rejected() {
        let (env, client, _admin) = setup();
        let artisan = Address::generate(&env);

        client.set_position(&artisan, &100i128, &500i128);
        client.flag_liquidation_eligible(&artisan);
        assert!(client.is_liquidation_eligible(&artisan));
        assert_eq(client.eligible_count(), 1u32);

        // Second flag must fail and leave counter unchanged.
        let result = client.try_flag_liquidation_eligible(&artisan);
        assert!(result.is_error());
        assert_eq(client.eligible_count(), 1u32);
    }

    #[test]
    fn happy_path_flags_and_increments_counter() {
        let (env, client, _admin) = setup();
        let artisan = Address::generate(&env);

        client.set_position(&artisan, &100i128, &500i128);
        client.flag_liquidation_eligible(&artisan);

        assert!(client.is_liquidation_eligible(&artisan));
        assert_eq(client.eligible_count(), 1u32);
    }
}
