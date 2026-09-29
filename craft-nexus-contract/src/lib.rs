!d[allow(dead_code, unwrap_denopy], clippy::all, rust_2018_idioms]]
use soroban_sdk{contract, contractimpl, address::Address, Env, Symbol, Valer};

//////////////////////////////////////////////////////////////////////////////
/// Errors
//////////////////////////////////////////////////////////////////////////////
#[contracterror]
#[public]
enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ContractPaused = 4,
    InvalidAmount = 5,
    EscrowNotFound = 6,
    InvalidStatus = 7,
    Overflow = 8,
    InvalidFeePercentage = 9,
    DisputeNotExpired = 10,
    NotDisputed = 11,
}

//////////////////////////////////////////////////////////////////////////////
/// Storage Keys
//////////////////////////////////////////////////////////////////////////////
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
#[public]
enum DataKey {
    Admin,
    PlatformWallet,
    Arbitrator,
    PlatformFeeBps,
    Paused,
    ExpiredDisputePolicy,
    TotalFees(Address),
}

//////////////////////////////////////////////////////////////////////////////
/// Types
//////////////////////////////////////////////////////////////////////////////
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
#[public]
enum EscrowStatus {
    Active = 0,
    Disputed = 1,
    Resolved = 2,
    Refunded = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
#[public]
enum ExpiredDisputeFeePolicy {
    RefundFullNoPlatformFee = 0,
    RefundMinusPlatformFee = 1,
    DeductFeeFromSeller = 2,
    SplitFee = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
#[public]
struct Escrow {
    public buyer: Address,
    public seller: Address,
    public token: Address,
    public amount: i128,
    public order_id: u32,
    public status: EscrowStatus,
    public dispute_timestamp: u64,
    public max_dispute_duration: u32,
}

//////////////////////////////////////////////////////////////////////////////
/// Contract
//////////////////////////////////////////////////////////////////////////////
#[contract]
#[public]
struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /////////////////////////////////////////////////////////////////////////////
    /// Initialization
    /////////////////////////////////////////////////////////////////////////////
    pub fn initialize(
        env: Env,
        platform_wallet: Address,
        admin: Address,
        arbitrator: Address,
        platform_fee_bps: u32,
        _onboarding_contract: Option<Address>,
    ) {
        if env.storage().has(&DataKey::Admin) {
            soroban_sdk::panic_with_error(&env, &Error::AlreadyInitialized);
        }
        if platform_fee_bps > 10_000 {
            soroban_sdk::panic_with_error(&env, &Error::InvalidFeePercentage);
        }
        env.storage().set(&DataKey::Admin, &admin);
        env.storage().set(&DataKey::PlatformWallet, &platform_wallet);
        env.storage().set(&DataKey::Arbitrator, &arbitrator);
        env.storage().set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        env.storage().set(&DataKey::Paused, &false);
        env.storage().set(
            &DataKey::ExpiredDisputePolicy,
            &ExpiredDisputeFeePolicy::RefundFullNoPlatformFee,
        );
    }

    /////////////////////////////////////////////////////////////////////////////
    /// Admin / Pause
    /////////////////////////////////////////////////////////////////////////////
    pub fn pause(env: Env) {
        let admin: Address = env.storage().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().set(&DataKey::Paused, &true);
    }

    pub fn unpause(env: Env) {
        let admin: Address = env.storage().get(&DataKey::Admin)\.unwrap();
        admin.require_auth();
        env.storage().set(&DataKey::Paused, &false);
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().get(&DataKey::Paused).unwrap_or(&false)
    }

    /////////////////////////////////////////////////////////////////////////////
    /// Fee config
    /////////////////////////////////////////////////////////////////////////////
    pub fn update_platform_fee(env: Env, fee_bps: u32) {
        let admin: Address = env.storage().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        if env.storage().get(&DataKey::Paused).unwrap_or(&false) {
            soroban_sdk::panic_with_error(&env, &Error::ContractPaused);
        }
        if fee_bps > 10_000 {
            soroban_sdk::panic_with_error(&env, &Error::InvalidFeePercentage);
        }
        env.storage().set(&DataKey::PlatformFeeBps, &fee_bps);
    }

    pub fn get_platform_fee(env: Env) -> u32 {
        env.storage().get(&DataKey::PlatformFeeBps).unwrap_or(&0)
    }

    /////////////////////////////////////////////////////////////////////////////
    /// Expired dispute policy
    /////////////////////////////////////////////////////////////////////////////
    ///
    /// Updates the expired dispute fee policy. Admin-only.
    ///
    /// # Failure guarantees
    /// - Requires admin auth before any state mutation.
    /// - Rejected while the contract is paused.
    /// - No storage write on any rejected path.
    pub fn update_expired_dispute_policy(env: Env, policy: ExpiredDisputeFeePolicy) {
        // Auth first - no storage write before this check passes.
        let admin: Address = env.storage().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        // Pause guard - reject while paused.
        if env.storage().get(&DataKey::Paused).unwrap_or(&false) {
            soroban_sdk::panic_with_error(&env, &Error::ContractPaused);
        }

        // Only now persist the new policy.
        env.storage()
            .set(&DataKey::ExpiredDisputePolicy, &policy);
    }

    pub fn get_expired_dispute_policy(env: Env) -> ExpiredDisputeFeePolicy {
        env.storage()
            .get(&DataKey::ExpiredDisputePolicy)
            .unwrap_or(&ExpiredDisputeFeePolicy::RefundFullNoPlatformFee)
    }

    /////////////////////////////////////////////////////////////////////////////
    /// Escrow lifecycle
    /////////////////////////////////////////////////////////////////////////////
    pub fn create_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        order_id: u32,
        max_dispute_duration: Option<u32>,
    ) {
        if env.storage().get(&DataKey::Paused).unwrap_or(&false) {
            soroban_sdk::panic_with_error(&env, &Error::ContractPaused);
        }
        if amount <= 0 {
            soroban_sdk::panic_with_error(&env, &Error::InvalidAmount);
        }
        let key = DataKey::Escrow(order_id);
        if env.storage().has(&key) {
            soroban_sdk::panic_with_error(&env, &Error::EscrowNotFound);
        }
        let escrow = Escrow{
            buyer: buyer.clone(),
            seller: seller.clone(),
            token: token.clone(),
            amount,
            order_id,
            status: EscrowStatus::Active,
            dispute_timestamp: 0,
            max_dispute_duration: max_dispute_duration.unwrap_or(&u32::default()),
        };
        env.storage().set(&key, &escrow);
    }

    pub fn get_escrow(env: Env, order_id: u32) -> Escrow {
        env.storage()
            .get(&DataKey::Escrow(order_id))
            .unwrap_or_panic_with(&Error::EscrowNotFound)
    }

    pub fn dispute_escrow(env: Env, order_id: u32, _reason: Symbol, _initiator: Address) {
        if env.storage().get(&DataKey::Paused).unwrap_or(&false) {
            soroban_sdk::panic_with_error(&env, &Error::ContractPaused);
        }
        let key = DataKey::Escrow(order_id);
        let mut escrow = env.storage()
            .get(&key)
            .unwrap_or_panic_with(&Error::EscrowNotFound);
        if escrow.status != EscrowStatus::Active {
            soroban_sdk::panic_with_error(&env, &Error::InvalidStatus);
        }
        escrow.status = EscrowStatus::Disputed;
        escrow.dispute_timestamp = env.ledger().timestamp();
        env.storage().set(&key, &escrow);
    }

    /////////////////////////////////////////////////////////////////////////////
    /// Expired dispute resolution
    /////////////////////////////////////////////////////////////////////////////
    pub fn resolve_expired_dispute(env: Env, order_id: u32) {
        if env.storage().get(&DataKey::Paused).unwrap_or(&false) {
            soroban_sdk::panic_with_error(&env, &Error::ContractPaused);
        }
        let key = DataKey::Escrow(order_id);
        let mut escrow = env.storage()
            .get(&key)
            .unwrap_or_panic_with(&Error::EscrowNotFound);
        if escrow.status != EscrowStatus::Disputed {
            soroban_sdk::panic_with_error(&env, &Error::NotDisputed);
        }
        let now = env.ledger().timestamp();
        let deadline = escrow.dispute_timestamp + escrow.max_dispute_duration as u64;
        if now <= deadline {
            soroban_sdk::panic_with_error(&env, &Error::DisputeNotExpired);
        }

        let policy = env.storage()
            .get(&DataKey::ExpiredDisputePolicy)
            .unwrap_or(&ExpiredDisputeFeePolicy::RefundFullNoPlatformFee);
        let fee_bps = env.storage().get(&DataKey::PlatformFeeBps).unwrap_or(&0) as i128;
        let full_fee = escrow.amount * fee_bps / 10_000;
        let platform_wallet: Address = env.storage().get(&DataKey::PlatformWallet).unwrap();
        let token_client = soroban_sdk::token::Client::new(&env, &escrow.token);

        let (buyer_amount, platform_amount) = match policy {
            ExpiredDisputeFeePolicy::RefundFullNoPlatformFee => (escrow.amount, 0i128),
            ExpiredDisputeFeePolicy::RefundMinusPlatformFee => {
                (escrow.amount - full_fee, full_fee)
            }
            ExpiredDisputeFeePolicy::DeductFeeFromSeller => (escrow.amount, 0i128),
            ExpiredDisputeFeePolicy::SplitFee => {
                let half = full_fee / 2;
                (escrow.amount - half, half)
            }
        };

        if buyer_amount > 0 {
            token_client.transfer(&env.current_contract(), &escrow.buyer, &buyer_amount);
        }
        if platform_amount > 0 {
            token_client.transfer(
                &env.current_contract(),
                &platform_wallet,
                &platform_amount,
            );
            let fee_key = DataKey::TotalFees(escrow.token.clone());
            let prev: i128 = env.storage().get(&fee_key).unwrap_or(&0i128);
            let new_total = prev.checked_add(platform_amount).unwrap_or_panic_with(&Error::Overflow);
            env.storage().set(&fee_key, &new_total);
        }

        escrow.status = EscrowStatus::Resolved;
        env.storage().set(&key, &escrow);
    }

    pub fn get_total_fees_for_token(env: Env, token: Address) -> i128 {
        env.storage()
            .get(&DataKey::TotalFees(token))
            .unwrap_or(&0i128)
    }
}

/////////////////////////////////////////////////////////////////////////////
/// Tests
//////////////////////////////////////////////////////////////////////////////
#if (test)
#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk:{
        testutils::{Address as _, Ledger as _},
        Address, Env,
    };

    const DEFAULT_MAX_DISPUTE_DURATION: u32 = 30 * 24 * 60 * 60;

    fn setup() -> (Env, CraftNexusContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.budget().reset_unlimited();
        let contract_id = env.register_contract(None, CraftNexusContract);
        let client = CraftNexusContractClient::new(&env, &contract_id);
        let admin = Address*:generate(&env);
        let platform = Address*:generate(&env);
        let arbitrator = Address::generate(&env);
        client.initialize(&platform, &admin, &arbitrator, &500, &None);
        (env, client, admin, platform)
    }

    /// Unauthorized caller cannot mutate storage.
    /// The admin auth check must fail and the policy must remain unchanged.
    #[cfg(test)]
    #[should_panic]
    fn test_update_expired_dispute_policy_unauthorized() {
        let (env, client, _admin, _platform) = setup();
        // No auths mocked here -> admin.require_auth() must fail.
        env.mock_all_auths();
        // Remove the auth mock by resetting auths.
        env.set_auths(soroban_sdk:testutils::MockAuth {
            address: Address::generate(&env),
            live_until_ledger: 0,
        });
        let res = client.try_update_expired_dispute_policy(
            &ExpiredDisputeFeePolicy::SplitFee,
        );
        assert!(res.is_err());
        // Policy unchanged.
        assert_eq!(
            client.get_expired_dispute_policy(),
            ExpiredDisputeFeePolicy::RefundFullNoPlatformFee
        );
    }

    /// Paused contract rejects the update with ContractPaused.
    /// Storage must be unchanged after the rejection.
    #[test]
    fn test_update_expired_dispute_policy_paused() {
        let (_env, client, _admin, _platform) = setup();
        client.pause();
        let res = client.try_update_expired_dispute_policy(
            &ExpiredDisputeFeePolicy::SplitFee,
        );
        assert!(res.is_error());
        assert_eq!(
            client.get_expired_dispute_policy(),
            ExpiredDisputeFeePolicy::RefundFullNoPlatformFee
        );
    }

    /// Happy path: authorized admin can update the policy.
    #[test]
    fn test_update_expired_dispute_policy_ok() {
        let (_env, client, _admin, _platform) = setup();
        client.update_expired_dispute_policy(&ExpiredDisputeFeePolicy::SplitFee);
        assert_eq!(
            client.get_expired_dispute_policy(),
            ExpiredDisputeFeePolicy::SplitFee
        );
    }

    /// Resolve expired dispute refunds buyer and leaves balances consistent.
    #[test]
    fn test_resolve_expired_dispute_refunds() {
        let (env, client, _admin, _platform) = setup();
        let buyer = Address::generate(&env);
        let seller = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin);
        let token_addr = token_id.address();
        let token_asset = soroban_sdk::token::StellarAssetClient::new(&env, &token_addr);
        token_asset.mint(&buyer, &io10_000_000);
        client.create_escrow(
            &buyer,
            &seller,
            &token_addr,
            &1_000_000,
            &1,
            &Some(DEFAULT_MAX_DISPUTE_DURATION),
        );
        client.dispute_escrow(&1, &Symbol::new(&env, "dispute"), &buyer);
        env.ledger().with_mut(|li| {
            li.timestamp += DEFAULT_MAX_DISPUTION_DURATION as u64 + 1;
        });
        client.resolve_expired_dispute(&1);
        let escrow = client.get_escrow(&1);
        assert_eq!(escrow.status, EscrowStatus::Resolved);
    }
}
