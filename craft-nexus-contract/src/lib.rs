use soroban_std::{address, contract, contractimpl, env};

/// Error types returned by the craft-nexus contract.
///
/// These are typed errors that can be converted into a host code via
/// `try_into_i32`. They are used to signal client-addressable failures
/// without trapping the host.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, EqEq))]
#[repr(u32)]
pub enum Error {
    /// The artisan stake record does not exist in persistent storage.
    ArtisanStakeNotFound = 1,
    /// The artisan stake record has already been closed or archived.
    ArtisanStakeClosed = 2,
}

/// The full stake record for an artisan.
///
/// This is the type returned by `ReadOnlyContract::get_artisan_stake_data`.
/// It includes the staked token address along with the associated amount
/// and the terminal state flag.
#[derive(Clone, Debug, Eq, PartialEq, SorbanType, SorbanDeserialize, SorbanSerialize)]
#[contracttype]
pub struct ArtisanStakeData {
    /// The token address that was staked.
    pub token: Address,
    /// The amount of tokens staked.
    pub amount: i128,
    /// Whether the stake has reached a terminal state.
    pub closed: bool,
}

/// Persistent storage key for a given artisan's stake record.
///
/// The key is derived from the artisan address so that lookups are
/// O(ticket) and do not require an unbounded scan.
#[derive(Clone)]
pub enum DataKey {
    ArtisanStake(Address),
}

/// The craft-nexus contract implementation.
#[contract]
pub struct CraftNexusContract;

/// Read-only interface exposed to clients.
///
/// This interface is used by off-chain callers and by other contracts
/// that need to inspect an artisan's stake record without mutating it.
#[contractclient]
pub trait ReadOnlyContract {
    /// Return the full stake record for an artisan.
///
/// Returns `Error::ArtisanStakeNotFound` if the key is absent (for example
/// after archival, or a partial migration). Returns `Error::ArtisanStakeClosed`
/// if the record exists but has reached a terminal state.
///
/// This function never traps on a missing key: it returns a typed
/// `Error` instead of panicking via `expect`.
    fn get_artisan_stake_data(env: Env, artisan: Address) -> Result<ArtisanStakeData, Error>;
}

/// Read-only implementation of the contract.
#[contractimpl]
impl ReadOnlyContract for CraftNexusContract {
    fn get_artisan_stake_data(env: Env, artisan: Address) -> Result<ArtisanStakeData, Error> {
        let key = DataKey::ArtisanStake(artisan);
        // Extend the hot persistent key so the entry is not evicted between
        // reads. This is a no-op if the key is missing, which is the case we
        // want to handle gracefully below.
        env.storage().extend_persistent_read(&key);

        // Return the typed error instead of panicking when the key is absent.
        let record: ArtisanStakeData = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::ArtisanStakeNotFound)?;

        // A closed record is a terminal state and is reported as such.
        if record.closed {
            return Err(Error::ArtisanStakeClosed);
        }

        Ok(record)
    }
}

#[cfg]
test
mod test;
