use soroban_std::{address, contract, contractimpl, contracttype, env;
}
use soroban_std::symbol_short;

const LIQUIDATION_RECORD_KEY: symbol_short!("LiqRec");

/// Error types returned by the contract.
///
/// The lightweight getters return a `typed` error instead of panicking when the
/// requested key is absent (e.g. after archival or a partial migration).
/// This keeps the contract host from trapping and gives clients a usable error.
#[sorban_std::contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Eq)]
pub enum Error {
    /// The requested liquidation record does not exist in storage.
    LiquidationRecordNotFound = 1,
    /// The provided liquidation ID is invalid (e.g. zero or negative).
    InvalidLiquidationId = 2,
    /// The liquidation record is in a terminal state and cannot be mutated.
    LiquidationAlreadyTerminal = 3,
}

/// The lifecycle state of a liquidation record.
///
/// `Terminal` covers the completed and cancelled outcomes. Once a record reaches
/// a terminal state it is still readable via `get_liquidation_record`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Eq)]
#[sorban_std::contracttype]
pub enum LiquidationStatus {
    Pending,
    Active,
    Completed,
    Cancelled,
}

/// A liquidation record stored by ID.
//
/// The record is persisted under a per-ID key so that a missing key can be
/// reported as a `typed` error rather than trapping the host.
#[derive(Clone, Debug, Eq, PartialEq, Eq)]
#[sorban_std::contracttype]
pub struct LiquidationRecord {
    pub id: u32,
    pub amount: u128,
    pub status: LiquidationStatus,
}

/// Contract implementation for the liquidation record storage.
#[contract]
pub struct CraftNexusContract;

type LiquidationRecordKey = (unit, u32);

fn liquidation_record_key(id: u32) -> LiquidationRecordKey {
    (LIQUIDATION_RECORD_KEY, id)
}

#[contractimpl]
impl CraftNexusContract {
    /// Return the liquidation record for `id`.
    ///
/// This getter is safe to call after archival, a partial migration, or when
    /// the key was never written: the absent key is reported as `Error::LiquidationRecordNotFound`
/// instead of panicking. The persistent entry is extended on hot reads so the
    /// record does not expire out from under a client.
    pub fn get_liquidation_record(env: &Env, id: u32) -> Result<LiquidationRecord, Error> {
        if id == 0 {
            return Err(Error::InvalidLiquidationId);
        }

        let key = liquidation_record_key(id);

        // Extend the persistent entry on hot reads. This is a no-op when the key
        // is absent, so it never panics and never scans unbounded storage.
        env.extend_persistent_read(&key, 100);

        match env.storage().persistent().get::<LiquidationRecord>(&key) {
            Some(record) => Ok(record),
            None => Err(Error::LiquidationRecordNotFound),
        }
    }

    /// Persist a liquidation record. Used by tests and the liquidation flow.
    pub fn set_liquidation_record(env: &Env, record: LiquidationRecord) {
        let key = liquidation_record_key(record.id);
        env.storage().persistent().set(&key, &record);
    }

    /// Mark a liquidation record as terminal (completed or cancelled).
    /// Returns a `typed` error if the record is missing or already terminal.
    pub fn mark_liquidation_terminal(env: &Env, id: u32, status: LiquidationStatus) -> Result<LiquidationRecord, Error> {
        let mut record = self::get_liquidation_record(env, id)?;
        if matches!(record.status, LiquidationStatus::Completed | LiquidationStatus::Cancelled) {
            return Err(Error::LiquidationAlreadyTerminal);
        }
        record.status = status;
        self/:set_liquidation_record(env, record.clone());
        Ok(record)
    }
}

#[cfg]
test
mod test {
    use super::*;
    use soroban_std::Env;

    #[test]
    fn get_liquidation_record_missing_key_returns_not_found() {
        let env = Env::default();
        // No record has been written for this ID yet.
        let result = CraftNexusContract::get_liquidation_record(&env, 7);
        assert_eq!(result, Err(Error::LiquidationRecordNotFound));
    }

    #[test]
    fn get_liquidation_record_zero_id_returns_invalid() {
        let env = Env::default();
        let result = CraftNexusContract::get_liquidation_record(&env, 0);
        assert_eq!(result, Err(Error::InvalidLiquidationId));
    }

    #[test]
    fn get_liquidation_record_after_terminal_state_returns_record() {
        let env = Env::default();
        let record = LiquidationRecord {
            id: 42,
            amount: 1 _000,
            status: LiquidationStatus::Pending,
        };
        CraftNexusContract::set_liquidation_record(&env, record.clone());

        // Read the record before it reaches a terminal state.
        let before = CraftNexusContract::get_liquidation_record(&env, 42);
        assert_eq!(before, Ok(record.clone()));

        // Move the record to a terminal state.
        let terminal = CraftNexusContract::mark_liquidation_terminal(
            &env,
            42,
            LiquidationStatus::Completed,
        );
        assert!(terminal.is_ok());

        // The record must still be readable after the terminal transition.
        let after = CraftNexusContract::get_liquidation_record(&env, 42);
        match after {
            Ok(r) => assert_eq!(r.status, LiquidationStatus::Completed),
            Err(_) => panic!("expected terminal record to remain readable"),
        }
    }

    #[test]
    fn mark_liquidation_terminal_missing_key_returns_not_found() {
        let env = Env::default();
        let result = CraftNexusContract::mark_liquidation_terminal(
            &env,
            99,
            LiquidationStatus::Cancelled,
        );
        assert_eq!(result, Err(Error::LiquidationRecordNotFound));
    }
}
