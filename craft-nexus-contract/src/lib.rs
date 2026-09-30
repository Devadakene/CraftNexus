#a[hard_error]
#a[hard_no_std]
//! Craft Nexus Contract
///
/// This library implements the core Craft Nexus escrow and liquidation
logic. The focus of this file is the lifecycle of an escrow and the
liquidation policy that governs how undercollateralized artisans are
treated.

/// Error codes returned by the contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(uint32)]
pub enum Error {
    /// The contract has already been initialized.
    AlreadyInitialized = 1,
    /// The contract has not been initialized.
    NotInitialized = 2,
    /// The caller is not authorized to perform the action.
    Unauthorized = 3,
    /// The platform is paused.
    ContractPaused = 4,
    /// An arithmetic overflow or underflow occurred.
    Overflow = 5,
    /// The supplied liquidation policy is invalid.
    InvalidPolicy = 6,
    /// The artisan is not undercollateralized.
    NotUnderCollateralized = 7,
    /// The artisan is not eligible for liquidation.
    NotLiquidationEligible = 8,
    /// The liquidation policy is disabled.
    LiquidationDisabled = 9,
    /// The grace period has not elapsed.
    GracePeriodNotElapsed = 10,
    /// The escrow does not exist.
    EscrowNotFound = 11,
    /// The escrow is in an invalid state for the operation.
    InvalidEscrowState = 12,
    /// The artisan has no stake.
    NoStake = 13,
    /// The amount is invalid.
    InvalidAmount = 14,
}

/// The lifecycle status of an artisan's stake.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(uint32)]
pub enum LiquidationStatus {
    /// The artisan is adequately collateralized.
    Healthy = 0,
    /// The artisan is undercollateralized.
    UnderCollateralized = 1,
    /// The artisan has been flagged as eligible for liquidation.
    LiquidationEligible = 2,
    /// The artisan's stake has been liquidated.
    Liquidated = 3,
}

/// The global liquidation policy.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LiquidationPolicy {
    /// Whether liquidation is enabled.
    pub enabled: bool,
    /// Maximum fraction of the deficit that may be seized, in basis points.
    pub max_seizure_bps: u32,
    /// Grace period before liquidation may be triggered, in seconds.
    pub grace_period_secs: u64,
}

/// A snapshot of an artisan's stake health.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeHealthSnapshot {
    /// The artisan this snapshot describes.
    pub artisan: Address,
    /// The current stake balance.
    pub current_stake: i128,
    /// The number of active obligations.
    pub active_obligations: u32,
    /// The required collateral.
    pub required_collateral: i128,
    /// The deficit, capped at zero.
    pub deficit: i128,
    /// The health ratio in basis points.
    pub health_ratio_bps: u32,
    /// The determined status.
    pub status: LiquidationStatus,
    /// The ledger timestamp when the snapshot was taken.
    pub timestamp: u64,
}

/// A liquidation record for auditing.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LiquidationRecord {
    /// The artisan whose stake was seized.
    pub artisan: Address,
    /// The amount seized.
    pub seized_amount: i128,
    /// The deficit at the time of liquidation.
    pub deficit: i128,
    /// The ledger timestamp of the liquidation.
    pub timestamp: u64,
}

/// The default grace period (two days).
pub const DEFAULT_LIQUIDATION_GRACE_PERIOD: u64 = 2 * 24 * 60 * 60;
/// The default maximum seizure fraction (50%).
pub const DEFAULT_MAX_SEIZURE_BPS: u32 = 5000;
/// The maximum allowed max seizure fraction (100%).
pub const MAX_SEIZURE_BPS: u32 = 10000;

/// The main contract.
#[no_std]
#[contract]
pub struct CraftNexusContract;

/// Storage keys.
#[no_std]
#[contracttype]
pub enum DataKey {
    Admin,
    Arbitrator,
    PlatformWallet,
    PlatformFeeBps,
    OnboardingContract,
    Paused,
    MinEscrowAmount(Address),
    MinReleaseWindow,
    EvidenceChallengeWindow,
    MinStakeRequired,
    Stake(Address),
    ActiveObligations(Address),
    StakeHealthSnapshot(Address),
    LiquidationStatus(Address),
    LiquidationPolicy,
    LiquidationRecord(Address),
}

#[no_std]
#[contractimpl]
pub impl CraftNexusContract {
    /// Initialize the contract.
    pub fn initialize(
        env: Env,
        platform_wallet: Address,
        admin: Address,
        arbitrator: Address,
        platform_fee_bps: u32,
        onboarding_contract: Option<Address>,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error(&env, Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Arbitrator, &arbitrator);
        env.storage().instance().set(&DataKey::PlatformWallet, &platform_wallet);
        env.storage().instance().set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        env.storage().instance().set(&DataKey::OnboardingContract, &onboarding_contract);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::MinReleaseWindow, &0);
        env.storage().instance().set(&DataKey::EvidenceChallengeWindow, &u64::0);
        env.storage().instance().set(&DataKey::MinStakeRequired, &i128:0);
        env.storage().instance().set(
            &DataKey::LiquidationPolicy,
            &LiquidationPolicy {
                enabled: true,
                max_seizure_bps: DEFAULT_MAX_SEIZURE_BPS,
                grace_period_secs: DEFAULT_LIQUIDATION_GRACE_PERIOD,
            },
        );
    }

    /// Return the admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    /// Return whether the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Pause the contract. Only the admin may call this.
    pub fn pause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error(&env, Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &true);
    }
}

    /// Unpause the contract. Only the admin may call this.
    pub fn unpause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error(&env, Error::Unauthorized);
        }
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    /// Set the minimum escrow amount for a token.
    pub fn set_min_escrow_amount(env: Env, token: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::MinEscrowAmount(token), &amount);
    }

    /// Set the minimum release window.
    pub fn set_min_release_window(env: Env, window: u64) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::MinReleaseWindow, &window);
    }

    /// Set the evidence challenge window.
    pub fn set_evidence_challenge_window(env: Env, window: u64) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::EvidenceChallengeWindow, &window);
    }

    /// Set the minimum stake required for artisans.
    pub fn set_min_stake_required(env: Env, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::MinStakeRequired, &amount);
    }

    /// Stake tokens for an artisan.
    pub fn stake_tokens(env: Env, artisan: Address, token: Address, amount: i128) {
        artisan.require_auth();
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            panic_with_error(&env, Error::ContractPaused);
        }
        if amount <= 0 {
            panic_with_error(&env, Error::InvalidAmount);
        }
        let key = DataKey::Stake(artisan.clone());
        let current: i128 = env.storage().instance().get(&key).unwrap_or(0);
        let updated = current.checked_add(amount).unwrap_or_else(|| {
            panic_with_error(&env, Error::Overflow);
        });
        env.storage().instance().set(&key, &updated);
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&artisan, &env.current_contract_address(), &amount);
    }

    /// Return the stake balance of an artisan.
    pub fn get_stake(env: Env, artisan: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::Stake(artisan))
            .unwrap_or(0)
    }

    /// Create an escrow.
    pub fn create_escrow(
        env: Env,
        buyer: Address,
        seller: Address,
        token: Address,
        amount: i128,
        _escrow_id: u32,
        _metadata: Option<Bytes>,
    ) {
        buyer.require_auth();
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            panic_with_error(&env, Error::ContractPaused);
        }
        if amount <= 0 {
            panic_with_error(&env, Error::InvalidAmount);
        }
        let min_amount: i128 = env.storage()
            .instance()
            .get(&DataKey::MinEscrowAmount(token.clone()))
            .unwrap_or(0);
        if amount < min_amount {
            panic_with_error(&env, Error::InvalidAmount);
        }
        let obligations_key = DataKey::ActiveObligations(seller.clone());
        let current: u32 = env.storage().instance().get(&obligations_key).unwrap_or(0);
        let updated = current.checked_add(1).unwrap_or_else(|| {
            panic_with_error(&env, Error::Overflow);
        });
        env.storage().instance().set(&obligations_key, &updated);
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&buyer, &env.current_contract_address(), &amount);
    }

    /// Evaluate the stake health of an artisan and persist the snapshot.
    pub fn evaluate_stake_health(env: Env, artisan: Address) -> StakeHealthSnapshot {
        let current_stake = Self::get_stake*env.clone(), artisan.clone());
        let active_obligations: u32 = env.storage()
            .instance()
            .get(&DataKey::ActiveObligations(artisan.clone()))
            .unwrap_or(0);
        let min_stake: i128 = env.storage()
            .instance()
            .get(&DataKey::MinStakeRequired)
            .unwrap_or(0);
        let required_collateral = min_stake;
        let deficit = if current_stake < required_collateral {
            required_collateral - current_stake
        } else {
            0
        };
        let health_ratio_bps = if required_collateral == 0 {
            10_000
        } else {
            ((current_stake * 10_000) / required_collateral) as u32
        };
        let status = if deficit > 0 {
            LiquidationStatus::UnderCollateralized
        } else {
            LiquidationStatus::Healthy
        };
        let snapshot = StakeHealthSnapshot {
            artisan: artisan.clone(),
            current_stale: current_stale,
            active_obligations: active_obligations,
            required_collateral,
            deficit,
            health_ratio_bps,
            status,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&DataKey::StakeHealthSnapshot(artisan.clone()), &snapshot);
        snapshot
    }

    /// Return the persisted stake health snapshot for an artisan.
    pub fn get_stake_health_snapshot(env: Env, artisan: Address) -> Option<StakeHealthSnapshot> {
        env.storage()
            .instance()
            .get(&DataKey::StakeHealthSnapshot(artisan))
    }

    /// Return the current liquidation policy.
    pub fn get_liquidation_policy(env: Env) -> LiquidationPolicy {
        env.storage()
            .instance()
            .get(&DataKey::LiquidationPolicy)
            .unwrap_or(LiquidationPolicy {
                enabled: true,
                max_seizure_bps: DEFAULT_MAX_SEIZURE_BPS,
                grace_period_secs: DEFAULT_LIQUIDATION_GRACE_PERIOD,
            })
    }

    /// Set the liquidation policy. Only the admin may call this.
    ///
    /// The function validates the inputs, checks authorization and the
    /// pause flag, and only then writes to storage. Any rejected input leaves
    /// storage unchanged.
    pub fn set_liquidation_policy(
        env: Env,
        admin: Address,
        max_seizure_bps: u32,
        grace_period_secs: u64,
        enabled: bool,
    ) {
        // Authorization first: a failed auth check must not mutate storage.
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error(&env, Error::Unauthorized);
        }

        // Respect the pause flag.
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            panic_with_error(&env, Error::ContractPaused);
        }

        // Validate the policy before writing.
        if max_seizure_bps > MAX_SEIZURE_BPS {
            panic_with_error(&env, Error::InvalidPolicy);
        }

        // Guard against any arithmetic overflow when deriving derived values.
        let _guarded = grace_period_secs.checked_add(0).unwrap_or_else(|| {
            panic_with_error(&env, Error::Overflow);
        });

        // All checks passed; commit the new policy.
        env.storage().instance().set(
            &DataKey::LiquidationPolicy,
            &LiquidationPolicy {
                enabled,
                max_seizure_bps,
                grace_period_secs,
            },
        );
    }

    /// Flag an artisan as liquidation-eligible. Only the admin may call this.
    pub fn flag_liquidation_eligible(env: Env, admin: Address, artisan: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error(&env, Error::Unauthorized);
        }
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            panic_with_error(&env, Error::ContractPaused);
        }
        let policy = Self::get_liquidation_policy(env.clone());
        if !policy.enabled {
            panic_with_error(&env, Error::LiquidationDisabled);
        }
        let snapshot = env.storage()
            .instance()
            .get(&DataKey::StakeHealthSnapshot(artisan.clone()))
            .unwrap_or_else(|| {
                panic_with_error(&env, Error::NotUnderCollateralized);
            });
        if snapshot.status != LiquidationStatus::UnderCollateralized {
            panic_with_error(&env, Error::NotUnderCollateralized);
        }
        let elapsed = env.ledger().timestamp().saturating_sub(snapshot.timestamp);
        if elapsed < policy.grace_period_secs {
            panic_with_error(&env, Error::GracePeriodNotElapsed);
        }
        env.storage()
            .instance()
            .set(&DataKey::LiquidationStatus(artisan), &LiquidationStatus::LiquidationEligible);
    }

    /// Return the liquidation status of an artisan.
    pub fn get_liquidation_status(env: Env, artisan: Address) -> LiquidationStatus {
        env.storage()
            .instance()
            .get(&DataKey::LiquidationStatus(artisan))
            .unwrap_or(0)
    }

    /// Trigger liquidation for an artisan. Only the admin may call this.
    pub fn trigger_liquidation(env: Env, admin: Address, artisan: Address) -> LiquidationRecord {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        if admin != stored_admin {
            panic_with_error(&env, Error::Unauthorized);
        }
        if env.storage().instance().get(&DataKey::Paused).unwrap_or(false) {
            panic_with_error(&env, Error::ContractPaused);
        }
        let policy = Self::get_liquidation_policy(env.clone());
        if !policy.enabled {
            panic_with_error(&env, Error::LiquidationDisabled);
        }
        let status = Self::get_liquidation_status(env.clone(), artisan.clone());
        if status != LiquidationStatus::LiquidationEligible {
            panic_with_error(&env, Error::NotLiquidationEligible);
        }
        let snapshot = env.storage()
            .instance()
            .get(&DataKey::StakeHealthSnapshot(artisan.clone()))
            .unwrap_or_else(|| {
                panic_with_error(&env, Error::NotUnderCollateralized);
            });
        let max_seizure = snapshot
            .deficit
            .checked_mul(policy.max_seizure_bps as i128)
            .unwrap_or_else(|| {
                panic_with_error(&env, Error::Overflow);
            })
            / 10_000;
        let current_stake = Self::get_stake(env.clone(), artisan.clone());
        let seized_amount = if max_seizure > current_stake {
            current_stake
        } else {
            max_seizure
        };
        let remaining = current_stake.checked_sub(seized_amount).unwrap_or_else(|| {
            panic_with_error(&env, Error::Overflow);
        });
        env.storage()
            .instance()
            .set(&DataKey::Stake(artisan.clone()), &remaining);
        let record = LiquidationRecord {
            artisan: artisan.clone(),
            seized_amount,
            deficit: snapshot.deficit,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .instance()
            .set(&DataKey::LiquidationRecord(artisan.clone()), &record);
        env.storage()
            .instance()
            .set(&DataKey::LiquidationStatus(artisan.clone()), &LiquidationStatus::Liquidated);
        record
    }

    /// Return the liquidation record for an artisan, if any.
    pub fn get_liquidation_record(env: Env, artisan: Address) -> Option<LiquidationRecord> {
        env.storage()
            .instance()
            .get(&DataKey::LiquidationRecord(artisan))
    }
}
