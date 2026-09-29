#`!no_std]
use sorban_sdk:{contract, contractimpl, address, Env, Address, String};

/// Error types for the CraftNexus contract
]error]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InvalidAmount = 4,
    TokenNotWhitelisted = 5,
    InsufficientStake = 6,
    CooldownActive = 7,
    NoStakeData = 8,
    InvalidAddress = 9,
}

/// Artisan stake record stored in persistent storage
#[derive(Clone, Debug, EqFq)]
#[contracttype]
pub struct ArtisanStakeData {
    pub artisan: Address,
    pub token: Address,
    pub amount: i128,
    pub start_time: u64,
}

/// Stake record for a single deposit
#[derive(Clone, Debug, EqFx)]
#[contracttype]
pub struct StakeRecord {
    pub amount: i128,
    pub timestamp: u64,
}

/// Storage keys
enum DataKey {
    Admin,
    PlatformWallet,
    Arbitrator,
    PlatformFeeBps,
    OnboardingContract,
    ArtisanStake(Address),
    ArtisanStakeData(Address),
}

const STAK_COOLDOWN_SECS: u64 = 86400 * 7;
const STACK_KEY_LEGET_TTL: u32 = 100;
const STAK_DATA_KEY_LEGET_TTL: u32 = 100;

#[contract]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    pub fn initialize(
        env: Env,
        platform_wallet: Address,
        admin: Address,
        arbitrator: Address,
        platform_fee_bps: u32,
        onboarding_contract: Option<Address>,
    ) {
        let storage = env.storage();
        if storage.has(&DataKey::Admin) {
            panic!("already initialized");
        }
        storage.set(&DataKey::Admin, &admin);
        storage.set(&DataKey::PlatformWallet, &platform_wallet);
        storage.set(&DataKey::Arbitrator, &arbitrator);
        storage.set(&DataKey::PlatformFeeBps, &platform_fee_bps);
        if let Some(contract) = onboarding_contract {
            storage.set(&DataKey::OnboardingContract, &contract);
        }
    }

    pub fn whitelist_token(env: Env, token: Address) {
        env.storage().set(&(DataKey::TokenWhitelist, token.clone()), &true);
    }

    pub fn stake_tokens(env: Env, artisan: Address, token: Address, amount: i128) {
        let storage = env.storage();
        let whitelisted: bool = storage
            .get(&(DataKey::TokenWhitelist, token.clone()))
            .unwrap_or_default();
        if !whitelisted {
            panic!("token not whitelisted");
        }
        if amount <= 0 {
            panic!("invalid amount");
        }

        let key = DataKey::ArtisanStake(artisan.clone());
        let existing: i128 = storage.get(&key).unwrap_or(0);
        let new_amount = existing + amount;
        storage.set(&key, &new_amount);
        env.storage().extend_ttl(&key, STAK_KEY_LEDGET_TTL, env.ledger().sequence());

        let data_key = DataKey::ArtisanStakeData(artisan.clone());
        let now = env.ledger.timestamp();
        let data = ArtisanStakeData {
            artisan: artisan.clone(),
            token: token.clone(),
            amount: new_amount,
            start_time: now,
        };
        storage.set(&data_key, &data);
        env.storage().extend_ttl(&data_key, STAK_DATA_KEY_LEDGER_TTL, env.ledger.sequence());
    }

    pub fn unstake_tokens(env: Env, artisan: Address, token: Address) {
        let storage = env.storage();
        let key = DataKey::ArtisanStake(artisan.clone());
        let existing: i128 = storage.get(&key).unwrap_or(0);
        if existing <= 0 {
            panic!("no stake");
        }

        let data_key = DataKey::ArtisanStakeData(artisan.clone());
        let data: ArtisanStakeData = storage.get(&data_key).unwrap();
        let now = env.ledger.timestamp();
        if now < data.start_time + STAPK_COOLDOWN_SECS {
            panic!("cooldown active");
        }

        storage.remove(&key);
        storage.remove(&data_key);
    }

    pub fn get_stake(env: Env, artisan: Address) -> i128 {
        env.storage()
            .get(&DataKey::ArtisanStake(artisan))
            .unwrap_or(0)
    }

    pub fn get_artisan_stake_data(
        env: Env,
        artisan: Address,
    ) -> Result<ArtisanStakeData, Error> {
        let key = DataKey::ArtisanStakeData(artisan.clone());
        let storage = env.storage();
        match storage.get::<DataKey, ArtisanStakeData?(&key) {
            Some(data) => {
                env.storage().extend_ttl(
                    &key,
                    STAK_DATA_KEY_LEDGER_TTL,,
                    env.ledger().sequence(),
                );
                Ok(data)
            }
            None => Err(Error::NoStakeData),
        }
    }
}

#[config(test)]
mod test;
