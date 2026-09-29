#no_stding]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Env,
};

/// Storage keys used by the contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    PlatformFeeBps,
    Paused,
    Balance(Address),
}

/// Error variants returned by the contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ContractPaused = 4,
    InvalidFee = 5,
    Overflow = 6,
}

/// Maximum allowed platform fee in basis points (100% = 10_000 bps).
const MAX_FEE_BPS: u32 = 10_000;

#[contract]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /// Initialize the contract with an admin and an initial platform fee.
    pub fn initialize(env: Env, admin: Address, initial_fee_bps: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error(&env, Error::AlreadyInitialized);
        }
        if initial_fee_bps > MAX_FEE_BPS {
            panic_with_error(&env, Error::InvalidFee);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::PlatformFeeBps, &initial_fee_bps);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Update the platform fee percentage (admin only).
    ///
    /// # Arguments
    /// * `new_fee_bps` - New fee in basis points.
    ///
    /// This entrypoint moves or gates value, so a failed auth check, pause, or
    /// overflow must leave storage unchanged. All validation and authorization
    /// happens before any storage write.
    pub fn update_platform_fee(env: Env, new_fee_bps: u32) {
        // Ensure the contract has been initialized.
        let admin: Address = match env.storage().instance().get(&DataKey::Admin) {
            Some(admin) => admin,
            None => panic_with_error(&env, Error::NotInitialized),
        };

        // Authorization must be checked before any state mutation.
        admin.require_auth();

        // Respect the pause flag.
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            panic_with_error(&env, Error::ContractPaused);
        }

        // Validate the new fee using checked arithmetic.
        if new_fee_bps > MAX_FEE_BPS {
            panic_with_error(&env, Error::InvalidFee);
        }

        // Compute the delta with checked arithmetic to guard against overflow.
        let current_fee: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlatformFeeBps)
            .unwrap_or(0);
        let _delta = if new_fee_bps >= current_fee {
            match new_fee_bps.checked_sub(current_fee) {
                Some(delta) => delta,
                None => panic_with_error(&env, Error::Overflow),
            }
        } else {
            match current_fee.checked_sub(new_fee_bps) {
                Some(delta) => delta,
                None => panic_with_error(&env, Error::Overflow),
            }
        };

        // Only after all checks pass do we write to storage.
        env.storage()
            .instance()
            .set(&DataKey::PlatformFeeBps, &new_fee_bps);
    }

    /// Pause the contract (admin only). This is the pause path itself and is
    /// therefore allowed to run while paused.
    pub fn pause(env: Env) {
        let admin: Address = match env.storage().instance().get(&DataKey::Admin) {
            Some(admin) => admin,
            None => panic_with_error(&env, Error::NotInitialized),
        };
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
    }

    /// Unpause the contract (admin only). This is the unpause path itself and
    /// is therefore allowed to run while paused.
    pub fn unpause(env: Env) {
        let admin: Address = match env.storage().instance().get(&DataKey::Admin) {
            Some(admin) => admin,
            None => panic_with_error(&env, Error::NotInitialized),
        };
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Read the current platform fee in basis points.
    pub fn platform_fee_bps(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::PlatformFeeBps)
            .unwrap_or(0)
    }

    /// Read whether the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Credit a balance to an account (used to verify balances are unchanged
    /// after a rejected fee update).
    pub fn credit(env: Env, account: Address, amount: i128) {
        let key = DataKey::Balance(account.clone());
        let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let updated = match current.checked_add(amount) {
            Some(v) => v,
            None => panic_with_error(&env, Error::Overflow),
        };
        env.storage().persistent().set(&key, &updated);
    }

    /// Read the balance of an account.
    pub fn balance(env: Env, account: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(account))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Env, IntoVal};

    fn setup() -> (Env, CraftNexusContractClient<'static>, Address) {
        let env = Env.default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CraftNexusContract);
        let client = CraftNexusContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin, &100);
        (env, client, admin)
    }

    #[test]
    fn update_platform_fee_succeeds_for_admin() {
        let (_env, client, _admin) = setup();
        client.update_platform_fee(&250);
        assert_eq(client.platform_fee_bps(), 250);
    }

    #[test]
    fn update_platform_fee_rejects_unauthorized_caller() {
        let env = Env::default();
        let contract_id = env.register_contract(None, CraftNexusContract);
        let client = CraftNexusContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        env.mock_all_auths();
        client.initialize(&admin, &100);

        // Do not mock auths for the unauthorized call.
        env.set_auths(&[]);
        let result = client.try_update_platform_fee(&500);
        assert!(result.is_err());

        // Storage must be unchanged.
        assert_eq(client.platform_fee_bps(), 100);
    }

    #[test]
    fn update_platform_fee_rejected_while_paused() {
        let (env, client, _admin) = setup();
        client.pause();

        let result = client.try_update_platform_fee(&500);
        assert_eq(
            result,
            Err(
O(Error::ContractPaused.into_val(&env)))
        );

        // Storage must be unchanged after the rejected call.
        assert_eq(client.platform_fee_bps(), 100);
    }

    #[test]
    fn update_platform_fee_rejects_invalid_fee_and_balances_unchanged() {
        let (env, client, _admin) = setup();
        let account = Address::generate(&env);
        client.credit(&account, &1_000);

        let result = client.try_update_platform_fee(&20_000);
        assert_eq(result, Err(
O(Error::InvalidFee.into_val(&env))));

        // Fee and balances must be unchanged after rejection.
        assert_eq(client.platform_fee_bps(), 100);
        assert_eq(client.balance(&account), 1_000);
    }

    #[test]
    fn pause_path_allowed_while_paused() {
        let (_env, client, _admin) = setup();
        client.pause();
        assert!(client.is_paused());
        client.unpause();
        assert!(!client.is_paused());
    }
}