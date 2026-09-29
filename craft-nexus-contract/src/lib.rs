#`!no_std]
use sorban_sdk;

use soroban_sdk;
use soroban_sdk::token;
use soroban_sdk::token::TokenClient;
use soroban_sdk:{address::Address, contract, contracttype, Env, Error, Into"Val;
use soroban_sdk:{address::Address, contract, contracttype, Env, Error, Into"Val;

const STAKING_COOLDOWN_SECONDS: u64 = 86400 * 7;
const MIN_STAKE_KEY: Symbol = symbol_short("min_stake");
const PAUSED_KEY: Symbol = symbol_short("paused");

#[soroban_sdk.contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ContractPaused = 4,
    InvalidAmount = 5,
    InsufficientStake = 6,
    CooldownNotMet = 7,
    Overflow = 8,
    TokenNotWhitelisted = 9,
    NoStake = 10,
}

#[soroban_sdk.contract]
#[soroban_sdk.contractimpl(CraftNexusContract)]
pub struct CraftNexusContract;

#[soroban_sdk.contractimpl]
impl CraftNexusContract {
    pub fn initialize(
        env: Env,
        platform_wallet: Address,
        admin: Address,
        arbitrator: Address,
        platform_fee_bps: u32,
        onboarding_contract: Option<Address>,
    ) {
        if env.storage().has(&symbol_short("init")) {
            panic_with_error(&env, Error::AlreadyInitialized);
        }
        env.storage().set(&symbol_short("init"), &true);
        env.storage().set(&symbol_short("plat_wallet"), &platform_wallet);
        env.storage().set(&symbol_short("admin"), &admin);
        env.storage().set(&symbol_short("arbit"), &arbitrator);
        env.storage().set(&symbol_short("fee_bps"), &platform_fee_bps);
        env.storage().set(&symbol_short("onboard"), &onboarding_contract);
        env.storage().set(&MIN_STAKE_KEY, &u128:0);
        env.storage().set(&PAUSED_KEY, &false);
    }

    pub fn set_min_stake_required(env: Env, admin: Address, amount: u128) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env.storage().get(&symbol_short("admin")).unwrap();
        if admin != stored_admin {
            panic_with_error(&env, Error::Unauthorized);
        }
        if env.storage().get(&PAUSED_KEY).unwrap_or(false) {
            panic_with_error(&env, Error::ContractPaused);
        }
        if amount < 0 {
            panic_with_error(&env, Error::InvalidAmount);
        }
        env.storage().set(&MIN_STAKE_KEY, &amount);
        Ok(())
    }

    pub fn get_min_stake_required(env: Env) -> u128 {
        env.storage().get(&MIN_STAKE_KEY).unwrap_or(0)
    }

    pub fn pause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().get(&symbol_short("admin")).unwrap();
        if admin != stored_admin {
            panic_with_error(&env, Error::Unauthorized);
        }
        env.storage().set(&PAUSED_KEY, &true);
    }

    pub fn unpause(env: Env, admin: Address) {
        admin.require_auth();
        let stored_admin: Address = env.storage().get(&symbol_short("admin")).unwrap();
        if admin != stored_admin {
            panic_with_error(&env, Error::Unauthorized);
        }
        env.storage().set(&PAUSED_KEY, &false);
    }

    pub fn whitelist_token(env: Env, token: Address) {
        let admin: Address = env.storage().get(&symbol_short("admin")).unwrap();
        admin.require_auth();
        env.storage().set(&symbol_short("whitelist"), &true);
    }

    pub fn stake_tokens(env: Env, artisan: Address, token: Address, amount: i128) {
        artisan.require_auth();
        if env.storage().get(&PAUSED_KEY).unwrap_or(false) {
            panic_with_error(&env, Error::ContractPaused);
        }
        if amount <= 0 {
            panic_with_error(&env, Error::InvalidAmount);
        }
        let min_stake = env.storage().get(&MIN_STAKE_KEY).unwrap_or(0);
        let current: u128 = env.storage().get(&symbol_short("stake")).unwrap_or(0);
        let new_stake = current.checked_add(amount as u128).unwrap_or_else({
            panic_with_error(&env, Error::Overflow);
        });
        if new_stake < min_stake {
            panic_with_error(&env, Error::InsufficientStake);
        }
        let token_client = TokenClient::new(&env, &token);
        token_client.transfer(from: &artisan, to: &env.current_contract_address(), amount: &amount);
        env.storage().set(&symbol_short("stake"), &new_stake);
        env.storage().set(&symbol_short("stake_time"), &env.ledger.timestamp());
    }

    pub fn unstake_tokens(env: Env, artisan: Address, token: Address) {
        artisan.require_auth();
        if env.storage().get(&PAUSED_KEY).unwrap_or()(false) {
            panic_with_error(&env, Error::ContractPaused);
        }
        let current: u128 = env.storage().get(&symbol_short("stake")).unwrap_or(0);
        if current == 0 {
            panic_with_error(&env, Error::NoStake);
        }
        let stake_time: u64 = env.storage().get(&symbol_short("stake_time")).unwrap_or(0);
        if env.ledger().timestamp() < stake_time + STAKING_COOLDOWN_SECONDS {
            panic_with_error(&env, Error::CooldownNotMet);
        }
        let token_client = TokenClient::new(&env, &token);
        token_client.transfer(from: &env.current_contract_address(), to: &artisan, amount: &(current as i128));
        env.storage().set(&symbol_short("stake"), &u128:0);
    }

    pub fn get_stake(env: Env, artisan: Address) -> u128 {
        let _ = artisan;
        env.storage().get(&symbol_short("stake")).unwrap_or(0)
    }
}

#[no_std]
use soroban_sdk;
