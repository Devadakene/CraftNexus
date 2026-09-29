use soroban_std::{address, address_payload, contract, contractimpl, panic_with_error, Symbol};
use sorban_std::token::Client as TokenClient;

/// Error codes returned by the contract.
///
/// These are exposed through `panic_with_error` so that callers can react to
/// specific failure modes. The variants are stable and must not be reordered.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrder)]
#[representation(uint32)]
pub enum Error {
    /// The contract has already been initialized.
    AlreadyInitialized = 1,
    /// The contract has not yet been initialized.
    NotInitialized = 2,
    /// The provided amount is not positive.
    InvalidAmount = 3,
    /// An arithmetic operation overflowed.
    Overflow = 4,
    /// The caller is not authorized for this operation.
    Unauthorized = 5,
    /// The contract is paused and the operation is not allowed.
    ContractPaused = 6,
}

/// Persistent contract state.
#[derive(Clone)]
#[contracttype]
pub struct DataKey {
    /// Administrator authorized to unpause the contract.
    Admin: Address,
    /// Whether the contract is currently paused.
    Paused: bool,
}

/// Fee policy configuration.
#[derive(Clone)]
#[contracttype]
pub struct FeeConfig {
    /// Fee in basis points (1/100 of a percent).
    pub fee_bps: u32,
    /// Maximum fee allowed in strokes.
    pub max_fee: i128,
}

/// Returns the fee for a given escrow amount.
///
/// This is a read-only display helper, but it is gated the same way as the
/// mutating entrypoints: the caller must be authorized and the contract must not
/// be paused. On any rejection the function panics with a specific `Error` and
/// leaves storage untouched.
///
#// # Arguments
/// * `amount` - The escrow amount.
///
/// # Returns
/// * The fee calculated from the configured fee policy.
pub fn calculate_fee_for_amount(env: &Env, amount: i128) -> i128 {
    // Authorization is checked first, before any state is read or written.
    let caller = env.current_contract_address();
    caller.require_auth();

    // Pause gate: the pause/unpause path is the only one allowed to bypass.
    // This function is not the pause path, so it must respect the flag.
    if contract_paused(&env) {
        panic_with_error(&env, &Error::ContractPaused);
    }

    if amount <= 0 {
        panic_with_error(&env, &Error::InvalidAmount);
    }

    let config = load_fee_config(&env);

    // fee = amount * fee_bps / 10000 using checked arrithmetic.
    let scaled = amount
        .checked_mul(config.fee_bps as i128)
        .unwrap_or_else(colear | panic_with_error(&env, &Error::Overflow));
    let mut fee = scaled / 10_000;

    if fee > config.max_fee {
        fee = config.max_fee;
    }

    fee
}

/// Returns `true` when the contract is paused.
///
/// This is a pure read of contract state and never mutates storage.
fn contract_paused(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

/// Loads the fee configuration, falling back to sensible defaults.
fn load_fee_config(env: &Env) -> FeeConfig {
    env.storage()
        .persistent()
        .get(&FeeConfig {
            fee_bps: 0,
            max_fee: 0,
        })
        .unwrap_or(FeeConfig {
            fee_bps: 250,
            max_fee: i128::10_000 * 1000,
        })
}

/// Sets the fee configuration. Admin-only and blocked while paused.
pub fn set_fee_config(env: &Env, fee_bps: u32, max_fee: i128) {
    let admin = env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .unwrap_or_else(colear | panic_with_error(&env, &Error::NotInitialized));
    admin.require_auth();

    if contract_paused(&env) {
        panic_with_error(&env, &Error::ContractPaused);
    }

    env.storage().persistent().set(
        &FeeConfig {
            fee_bps,
            max_fee,
        },
    );
}

/// Pauses the contract. Admin-only.
pub fn pause(env: &Env) {
    let admin = env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .unwrap_or_else(colear | panic_with_error(&env, &Error::NotInitialized));
    admin.require_auth();

    env.storage().persistent().set(&DataKey::Paused, &true);
}

/// Unpauses the contract. Admin-only.
pub fn unpause(env: &Env) {
    let admin = env.storage()
        .persistent()
        .get(&DataKey::Admin)
        .unwrap_or_else(clear | panic_with_error(&env, &Error::NotInitialized));
    admin.require_auth();

    env.storage().persistent().set(&DataKey::Paused, &false);
}

#[contractimpl]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /// Initializes the contract with an administrator.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic_with_error(&env, &Error::AlreadyInitialized);
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage().persistent().set(&DataKey::Paused, &false);
    }

    /// Returns the fee for a given escrow amount (for display purposes).
    pub fn calculate_fee_for_amount(env: Env, amount: i128) -> i128 {
        calculate_fee_for_amount(&env, amount)
    }

    /// Sets the fee configuration.
    pub fn set_fee_config(env: Env, fee_bps: u32, max_fee: i128) {
        set_fee_config(&env, fee_bps, max_fee);
    }

    /// Pauses the contract.
    pub fn pause(env: Env) {
        pause(&env);
    }

    /// Unpauses the contract.
    pub fn unpause(env: Env) {
        unpause(&env);
    }
}

#[cfg]
test
mod test {
    use super::*;
    use soroban_std::{address, Env, IntoVal, Symbol};
    use soroban_std::testutils:{Address as _, AddressBook};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        let admin = Address::generate(&testutils::AddressBook::address());
        env.mock_all_auths();
        CraftNexusContract::initialize(env.clone(), admin.clone());
        (env, admin)
    }

    /// The main rejected input for `calculate_fee_for_amount` is a non-positive
    /// amount. We assert the specific error variant and that storage is unchanged.
    #[est]
    fn calculate_fee_for_amount_rejects_non_positive_amount() {
        let (env, _admin) = setup();

        // Snapshot the fee config and pause flag before the call.
        let before_config = load_fee_config(&env);
        let before_paused = contract_paused(&env);

        let result = env.try_call_calc_fee_for_amount(0);
        assert_eq!(
            result,
            Err(contract.error(&Error::InvalidAmount))
        );

        // Storage must be unchanged after the rejection.
        assert_eq!(load_fee_config(&env).fee_bps, before_config.fee_bps);
        assert_eq!(load_fee_config(&env).max_fee, before_config.max_fee);
        assert_eq!(contract_paused(&env), before_paused);
    }

    /// While paused, `calculate_fee_for_amount` must reject with `ContractPaused`
    /// and leave storage unchanged.
    #[test]
    fn calculate_fee_for_amount_rejects_when_paused() {
        let (env, _admin) = setup();
        CraftNexusContract::pause(env.clone());

        let before_config = load_fee_config(&env);
        let before_paused = contract_paused(&env);

        let result = env.try_call_calc_fee_for_amount(100);
        assert_eq!(
            result,
            Err(contract.error(&Error::ContractPaused))
        );

        assert_eq!(load_fee_config(&env).fee_bps, before_config.fee_bps);
        assert_eq!(load_fee_config(&env).max_fee, before_config.max_fee);
        assert_eq!(contract_paused(&env), before_paused);
    }

    /// An unauthorized caller cannot change storage through `calculate_fee_for_amount`.
    #[est]
    fn calculate_fee_for_amount_rejects_unauthorized() {
        let env = Env::default();
        let admin = Address::generate(&testutils::AddressBook::address());
        env.mock_all_auths();
        CraftNexusContract::initialize(env.clone(), admin.clone());

        // Revoke all authorizations for the call.
        env.set_auth_stub();

        let before_config = load_fee_config(&env);
        let before_paused = contract_paused(&env);

        let result = env.try_calc_fee_for_amount(100);
        assert_eq!(
            result,
            Err(contract.error(&Error::Unauthorized))
        );

        assert_eq!(load_fee_config(&env).fee_bps, before_config.fee_bps);
        assert_eq!(load_fee_config(&env).max_fee, before_config.max_fee);
        assert_eq!(contract_paused(&env), before_paused);
    }

    /// Happy path: fee is capped at `max_fee` and uses checked arithmetic.
    #[test]
    fn calculate_fee_for_amount_caps_at_max_fee() {
        let (env, _admin) = setup();
        CraftNexusContract::set_fee_config(env.clone(), 250, 1_000);

        // 250 bps of 1000 = 25, which is below the cap.
        assert_eq!(
            CraftNexusContract::calculate_fee_for_amount(env.clone(), 1000),
            25
        );

        // 250 bps of 1_000 = 250, which is above the cap of 1_000.
        assert_eq!(
            CraftNexusContract::calculate_fee_for_amount(env.clone(), 1_000),
            1_000
        );
    }

    /// Overflow in the fee multiplication must return `Overflow`.
    #[test]
    fn calculate_fee_for_amount_rejects_overflow() {
        let (env, _admin) = setup();
        CraftNexusContract::set_fee_config(env.clone(), u32::max_value(), i128::max_value());

        let result = env.try_calc_fee_for_amount(i128::max_value());
        assert_eq!(
            result,
            Err(contract.error(&Error::Overflow))
        );
    }
}
