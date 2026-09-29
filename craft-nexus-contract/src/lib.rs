#no_std]

use soroban_sdk;
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Env,
};

/// Storage keys used by the contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Paused,
    /// Tracks whether an artisan is flagged as liquidation-eligible.
    LiquidationEligible(Address),
    /// Total amount of collateral held for an artisan.
    Collateral(Address),
    /// Aggregate count of flagged artisans.
    FlaggedCount,
}

/// Error variants returned by the contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Caller is not the admin.
    Unauthorized = 1,
    /// Contract is paused and the entrypoint is not the pause path.
    ContractPaused = 2,
    /// Arithmetic overflow/underflow while updating counters or amounts.
    Overflow = 3,
    /// Artisan is not under-collateralized and cannot be flagged.
    NotUnderCollateralized = 4,
    /// Artisan is already flagged as liquidation-eligible.
    AlreadyFlagged = 5,
}

#[contract]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /// Initialize the contract with an admin.
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::FlaggedCount, &0u32);
    }

    /// Pause or unpause the platform. This is the pause/unpause path itself,
    /// so it is exempt from the pause check.
    pub fn set_paused(env: Env, admin: Address, paused: bool) {
        admin.require_auth();
        Self::require_admin(&env, &admin);
        env.storage().instance().set(&DataKey::Paused, &paused);
    }

    /// Record collateral for an artisan. Used by tests and callers to set up
    /// the under-collateralization precondition.
    pub fn set_collateral(env: Env, admin: Address, artisan: Address, amount: i128) {
        admin.require_auth();
        Self::require_admin(&env, &admin);
        if amount < 0 {
            panic_with_error!(&env, Error::Overflow);
        }
        env.storage()
            .persistent()
            .set(&DataKey::Collateral(artisan), &amount);
    }

    /// Flag an under-collateralized artisan as liquidation-eligible.
    ///
    /// Value-gating entrypoint: auth, pause, and arithmetic checks all run
    /// before any storage write so a failure leaves storage unchanged.
    pub fn flag_liquidation_eligible(env: Env, admin: Address, artisan: Address) {
        // 1. Auth check on the intended role (admin).
        admin.require_auth();
        Self::require_admin(&env, &admin);

        // 2. Pause check.
        if Self::is_paused(&env) {
            panic_with_error!(&env, Error::ContractPaused);
        }

        // 3. Reject already-flagged artisans before mutating state.
        if env
            .storage()
            .persistent()
            .get::DataKey, bool>(&DataKey::LiquidationEligible(artisan.clone()))
            .unwrap_or(false)
        {
            panic_with_error!(&env, Error::AlreadyFlagged);
        }

        // 4. Reject artisans that are not under-collateralized.
        let collateral: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Collateral(artisan.clone()))
            .unwrap_or(0);
        if collateral > 0 {
            panic_with_error!(&env, Error::NotUnderCollateralized);
        }

        // 5. Checked counter update before any write.
        let flagged_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::FlaggedCount)
            .unwrap_or(0);
        let new_count = flagged_count
            .checked_add(1)
            .unwrap_or_else(|| panic_with_error!(&env, Error::Overflow));

        // All checks passed: perform storage writes.
        env.storage()
            .persistent()
            .set(&DataKey::LiquidationEligible(artisan.clone()), &true);
        env.storage()
            .instance()
            .set(&DataKey::FlaggedCount, &new_count);

        env.events()
            .publish((symbol_short!("flag_liq"), artisan), new_count);
    }

    /// Read whether an artisan is flagged as liquidation-eligible.
    pub fn is_liquidation_eligible(env: Env, artisan: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::LiquidationEligible(artisan))
            .unwrap_or(false)
    }

    /// Read the aggregate flagged count.
    pub fn flagged_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::FlaggedCount)
            .unwrap_or(0)
    }

    /// Read the paused flag.
    pub fn is_paused(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Internal helper: ensure the caller matches the stored admin.
    fn require_admin(env: &Env, caller: &Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(env, Error::Unauthorized));
        if admin != *caller {
            panic_with_error!(env, Error::Unauthorized);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk:testutils::Address as _;
    use soroban_sdk:{Env, IntoVal};

    fn setup() -> (Env, CraftNexusContractClient<'static>, Address) {
        let env = Env::default();
        let contract_id = env.register_contract(None, CraftNexusContract);
        let client = CraftNexusContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.init(&admin);
        (env, client, admin)
    }

    #[test]
    fn unauthorized_caller_cannot_flag_and_storage_unchanged() {
        let (env, client, _admin) = setup();
        let attacker = Address::generate(&env);
        let artisan = Address::generate(&env);

        env.mock_all_auths();

        let before_count = client.flagged_count();
        let before_eligible = client.is_liquidation_eligible(&artisan);

        let result = client.try_flag_liquidation_eligible(&attacker, &artisan);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));

        // Storage must be unchanged after rejection.
        assert_eq!(client.flagged_count(), before_count);
        assert_eq!(client.is_liquidation_eligible(&artisan), before_eligible);
    }

    #[test]
    fn rejected_while_paused_and_balances_unchanged() {
        let (env, client, admin) = setup();
        let artisan = Address::generate(&env);

        env.mock_all_auths();

        client.set_paused(&admin, &true);
        assert!(client.is_paused());

        let before_count = client.flagged_count();
        let before_eligible = client.is_liquidation_eligible(&artisan);

        let result = client.try_flag_liquidation_eligible(&admin, &artisan);
        assert_eq!(result, Err(Ok(Error::ContractPaused)));

        assert_eq!(client.flagged_count(), before_count);
        assert_eq!(client.is_liquidation_eligible(&artisan), before_eligible);
    }

    #[test]
    fn already_flagged_returns_error_and_balances_unchanged() {
        let (env, client, admin) = setup();
        let artisan = Address::generate(&env);

        env.mock_all_auths();

        client.flag_liquidation_eligible(&admin, &artisan);
        let count_after_first = client.flagged_count();

        let result = client.try_flag_liquidation_eligible(&admin, &artisan);
        assert_eq!(result, Err(Ok(Error::AlreadyFlagged)));

        assert_eq!(client.flagged_count(), count_after_first);
        assert!(client.is_liquidation_eligible(&artisan));
    }

    #[test]
    fn not_under_collateralized_returns_error_and_balances_unchanged() {
        let (env, client, admin) = setup();
        let artisan = Address::generate(&env);

        env.mock_all_auths();

        client.set_collateral(&admin, &artisan, &500i128);

        let before_count = client.flagged_count();
        let before_eligible = client.is_liquidation_eligible(&artisan);

        let result = client.try_flag_liquidation_eligible(&admin, &artisan);
        assert_eq!(result, Err(Ok(Error::NotUnderCollateralized)));

        assert_eq!(client.flagged_count(), before_count);
        assert_eq!(client.is_liquidation_eligible(&artisan), before_eligible);
    }

    #[test]
    fn happy_path_flags_under_collateralized_artisan() {
        let (env, client, admin) = setup();
        let artisan = Address::generate(&env);

        env.mock_all_auths();

        assert_eq!(client.flagged_count(), 0);
        assert!(!client.is_liquidation_eligible(&artisan));

        client.flag_liquidation_eligible(&admin, &artisan);

        assert_eq!(client.flagged_count(), 1);
        assert!(client.is_liquidation_eligible(&artisan));
    }

    #[test]
    fn pause_path_itself_is_not_blocked_by_pause() {
        let (env, client, admin) = setup();
        env.mock_all_auths();

        client.set_paused(&admin, &true);
        assert!(client.is_paused());

        // Unpausing must still work while paused.
        client.set_paused(&admin, &false);
        assert!(!client.is_paused());
    }

    #[test]
    fn overflow_on_flagged_count_returns_error() {
        let (env, client, admin) = setup();
        let artisan = Address::generate(&env);

        env.mock_all_auths();

        // Force the counter to u32::MAX to trigger checked_add overflow.
        env.as_contract(&client.address, || {
            env.storage()
                .instance()
                .set(&DataKey::FlaggedCount, &u32::MAX);
        });

        let before_eligible = client.is_liquidation_eligible(&artisan);

        let result = client.try_flag_liquidation_eligible(&admin, &artisan);
        assert_eq!(result, Err(Ok(Error::Overflow)));

        // Storage must be unchanged after overflow rejection.
        assert_eq!(client.flagged_count(), u32::MAX);
        assert_eq!(client.is_liquidation_eligible(&artisan), before_eligible);
    }

    #[test]
    fn error_variant_serializes_as_expected() {
        let env = Env::default();
        let val = Error::ContractPaused.into_val(&env);
        let _ = val;
    }
}
