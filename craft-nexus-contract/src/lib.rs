use soroban_std::{address, address::Address, contract, contracttype, env};

/// Error types for the Craft Nexus contract.
#[contracterror]
#[sorban_std::contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrder, Xdr))
#[no_std]
pub enum Error {
    /// The stake health snapshot key is missing.
    NotFound = 1,
    /// The artisan address is not valid.
    InvalidArtisan = 2,
    /// The caller is not authorized.
    Unauthorized = 3,
    /// The stake health snapshot is not available.
    SnapshotMissing = 4,
}

/// Health status of an artisan's stake.
#[type]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrder, Xdr)]
#[no_std]
pub enum StakeHealth {
    Healthy,
    Warning,
    Critical,
}

/// Persisted health snapshot for an artisan.
#[type]
#[derive(Clone, Debug, Eq, PartialEq, Xdr)]
#[no_std]
pub struct StakeHealthSnapshot {
    pub artisan: Address,
    pub stake: i128,
    pub status: StakeHealth,
    pub updated_at: u64,
}

/// Storage key for the persisted health snapshot of an artisan.
#[derive(Clone)]
#[no_std]
pub enum DataKey {
    StakeHealthSnapshot(Address),
}

/// Contract implementation.
#[sorban_std::contract]
#[no_std]
pub struct CraftNexusContract;

#[sorban_std::contractimp]
#[no_std]
impl CraftNexusContract {
    /// Return the persisted health snapshot for an artisan.
    ///
    /// Returns `None` if `evaluate_stake_health` has never been called for the
    /// given artisan, or if the key was removed (e.g. archival or partial migration).
    /// This getter must not trap when the storage key is absent.
    pub fn get_stake_health_snapshot(env: Env, artisan: Address) -> Option<StakeHealthSnapshot> {
        let key = DataKey::StakeHealthSnapshot(artisan);
        env.storage().persistent().get(&key)
    }

    /// Read the persisted health snapshot and return a typed error when absent.
    ///
    /// Uses `extend_persistent_read` on the hot persistent key so callers do not
    /// pay for an unbounded scan and do not trap on a missing key.
    pub fn get_stake_health_snapshot_or_error(
        env: Env,
        artisan: Address,
    ) -> Result<StakeHealthSnapshot, Error> {
        let key = DataKey::StakeHealthSnapshot(artisan);
        match env.storage().persistent().get(&key) {
            Some(snapshot) => {
                env.storage().extend_persistent_read(&key);
                Ok(snapshot)
            }
            None => Err(Error::SnapshotMissing),
        }
    }

    /// Persist a health snapshot for an artisan.
    pub fn evaluate_stake_health(
        env: Env,
        artisan: Address,
        stake: i128,
        status: StakeHealth,
    ) {
        let key = DataKey::StakeHealthSnapshot(artisan.clone());
        let snapshot = StakeHealthSnapshot {
            artisan,
            stake,
            status,
            updated_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&key, &snapshot);
    }
}

#[cfg]
test
mod test;
