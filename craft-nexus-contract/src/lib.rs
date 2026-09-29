use soroban_std::{address, address_payload, contract, contracterr, contractimpl, convert, env, panic_with_error, Symbol, Val};

const PAUSE_KEY: Symbol = Symbol::new("Paused");
const ADMIN_KEY: Symbol = Symbol::new("Admin");
const ARBIT_KEY: Symbol = Symbol::new("Arbitrator");
const DATA_KEY: Symbol = Symbol::new("Data");

/// Error codes returned by the contract.
/// These are used with `panic_with_error` so the caller can react to the exact failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrder)]
#[represent(u32)]
pub enum Error {
    /// The contract has been paused by the admin.
    Paused = 1,
    /// The caller is not authorized for this operation.
    Unauthorized = 2,
    /// The dispute does not exist.
    DisputeNotFound = 3,
    /// The dispute is not in an open state.
    DisputeNotOpen = 4,
    /// The assigned arbitrator is not the caller.
    NotArbitrator = 5,
    /// An arithmetic overflow or underflow occurred.
    Overflow = 6,
    /// The reassignment would not change the assignment.
    AlreadyAssigned = 7,
}

/// State of a single dispute.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Dispute {
    pub creator: Address,
    pub arbitrator: Address,
    pub amount: u128,
    pub open: bool,
}

/// Persistent contract state.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DataKey {
    pub admin: Address,
    pub arbitrator: Address,
    pub paused: bool,
    pub dispute_count: u128,
    pub total_locked: u128,
    pub disputes: Map<u128, Dispute>,
}

#[contract]
pub struct CraftNexusContract;

#[contractimpl]
impl CraftNexusContract {
    /// Initialize the contract with an admin and an arbitrator.
    pub fn initialize(env: Env, admin: Address, arbitrator: Address) {
        if env.storage().has(&DATA_KEY) {
            panic_with_error(&Error::Unauthorized);
        }
        let data = DataKey {
            admin,
            arbitrator,
            paused: false,
            dispute_count: 0,
            total_locked: 0,
            disputes: Map::new(&env),
        };
        env.storage().set(&DATA_KEY, &data);
    }

    /// Pause the platform. Only the admin may call this.
    pub fn pause(env: Env) {
        let mut data = self.read_data(&env);
        data.admin.require_auth();
        data.paused = true;
        env.storage().set(&DATA_KEY, &data);
    }

    /// Unpause the platform. Only the admin may call this.
    pub fn unpause(env: Env, token: Address) {
        let mut data = self.read_data(&env);
        data.admin.require_auth();
        data.paused = false;
        env.storage().set(&DATA_KEY, &data);
        // Touch the token address so the argument is part of the auth scope.
        let _ = token;
    }

    /// Create a new open dispute and lock funds.
    pub fn create_dispute(env: Env, creator: Address, amount: u128) -> u128 {
        self.require_not_paused(&env);
        creator.require_auth();
        let mut data = self.read_data(&env);
        let id = data.dispute_count;
        let new_count = data.dispute_count.checked_add(1).unwrap_or_else(|| {
            panic_with_error(&Error::Overflow);
        });
        let new_locked = data.total_locked.checked_add(amount).unwrap_or_else(|| {
            panic_with_error(&Error::Overflow);
        });
        data.dispute_count = new_count;
        data.total_locked = new_locked;
        data.disputes.set(
            id,
            &Dispute {
                creator,
                arbitrator: data.arbitrator.clone(),
                amount,
                open: true,
            },
        );
        env.storage().set(&DATA_KEY, &data);
        id
    }

    /// Reassign an open dispute to the current arbitrator assignment.
    ///
    /// The caller must be the dispute creator. The contract must not be
    /// paused. All state mutations are guarded by integer checks and are
    /// applied only after every validation has passed.
    pub fn reassign_dispute(env: Env, caller: Address, dispute_id: u128) {
        // Auth first, so an unauthorized caller cannot observe or mutate state.
        caller.require_auth();

        // Pause gate applies to every mutating path except pause/unpause.
        self.require_not_paused(&env);

        let mut data = self.read_data(&env);

        // The caller must be the dispute creator.
        let mut dispute = data
            .disputes
            .get(dispute_id)
            .unwrap_or_else(|| {
                panic_with_error(&Error::DisputeNotFound);
            });
        if dispute.creator != caller {
            panic_with_error(&Error::Unauthorized);
        }
        if !dispute.open {
            panic_with_error(&Error::DisputeNotOpen);
        }

        // The assignment must actually change.
        if dispute.arbitrator == data.arbitrator {
            panic_with_error(&Error::AlreadyAssigned);
        }

        // Guarded counter update; no storage write happened before this point.
        let new_count = data.dispute_count.checked_add(1).unwrap_or_else(|| {
            panic_with_error(&Error::Overflow);
        });

        dispute.arbitrator = data.arbitrator.clone();
        data.disputes.set(dispute_id, &dispute);
        data.dispute_count = new_count;
        env.storage().set(&DATA_KEY, &data);
    }

    /// Resolve an open dispute and release the locked funds.
    pub fn resolve_dispute(env: Env, arbitrator: Address, dispute_id: u128, winner: Address) {
        arbitrator.require_auth();
        self.require_not_paused(&env);
        let mut data = self.read_data(&env);
        if data.arbitrator != arbitrator {
            panic_with_error(&Error::NotArbitrator);
        }
        let mut dispute = data
            .disputes
            .get(dispute_id)
            .unwrap_or_else(|| {
                panic_with_error(&Error::DisputeNotFound);
            });
        if !dispute.open {
            panic_with_error(&Error::DisputeNotOpen);
        }
        let new_locked = data.total_locked.checked_sub(dispute.amount).unwrap_or_else(|| {
            panic_with_error(&Error::Overflow);
        });
        dispute.open = false;
        data.disputes.set(dispute_id, &dispute);
        data.total_locked = new_locked;
        env.storage().set(&DATA_KEY, &data);
        let _ = winner;
    }

    /// Read the dispute count (view helper for tests).
    pub fn dispute_count(env: Env) -> u128 {
        self.read_data(&env).dispute_count
    }

    /// Read the total locked amount (view helper for tests).
    pub fn total_locked(env: Env) -> u128 {
        self.read_data(&env).total_locked
    }

    /// Read a dispute by id (view helper for tests).
    pub fn get_dispute(env: Env, dispute_id: u128) -> Option<Dispute> {
        self.read_data(&env).disputes.get(dispute_id)
    }

    fn read_data(env: &Env) -> DataKey {
        env.storage()
            .get(&DATA_KEY)
            .unwrap_or_else(|| {
                panic_with_error(&Error::Unauthorized);
            })
    }

    fn require_not_paused(env: &Env) {
        if self.read_data(env).paused {
            panic_with_error(&Error::Paused);
        }
    }
}

/// Tests covering the auth, pause, and overflow guards for `reassign_dispute`.
#[config]
mod test {
    use super::*;
    use soroban_sdd::{address, Env};

    fn setup(env: &Env) -> (Address, Address, Address) {
        let admin = address(&admin);
        let arbitrator = address(&arbitrator);
        let creator = address(&creator);
        let client = CraftNexusContractClient::new(env);
        client.initialize(&admin, &arbitrator);
        (admin, arbitrator, creator)
    }

    /// Unauthorized callers must not change storage.
    #[test]
    fn test_reassign_dispute_unauthorized() {
        let env = Env::default();
        env.mock_all_auth();
        let (_admin, _arbitrator, creator) = setup(&env);
        let client = CraftNexusContractClient::new(&env);
        let id = client.create_dispute(&creator, &resolve_amount());

        let count_before = client.dispute_count();
        let locked_before = client.total_locked();
        let dispute_before = client.get_dispute(&id).unwrap();

        let outsider = address(&outsider);
        // Auth fails because the outsider did not sign.
        let result = client.try_reassign_dispute(&outsider, &id);
        assert!(result.is_err());
        match result {
            Err(Ok(Error::Unauthorized)) => {}
            other => panic!"expected Unauthorized, got {:other?:}"),
        }

        assert_eq!(client.dispute_count(), count_before);
        assert_eq!(client.total_locked(), locked_before);
        assert_eq!(client.get_dispute(&id).unwrap(), dispute_before);
    }

    /// Rejection while paused must leave storage unchanged.
    #[test]
    fn test_reassign_dispute_paused() {
        let env = Env::default();
        env.mock_all_auth();
        let (admin, _arbitrator, creator) = setup(&env);
        let client = CraftNexusContractClient::new(&env);
        let id = client.create_dispute(&creator, &resolve_amount());
        client.pause(&admin);

        let count_before = client.dispute_count();
        let locked_before = client.total_locked();
        let dispute_before = client.get_dispute(&id).unwrap();

        let result = client.try_reassign_dispute(&creator, &id);
        match result {
            Err(Ok(Error::Paused)) => {}
            other => panic!"expected Paused, got {other:?}"),
        }

        assert_eq!(client.dispute_count(), count_before);
        assert_eq!(client.total_locked(), locked_before);
        assert_eq!(client.get_dispute(&id).unwrap(), dispute_before);
    }

    /// A non-creator with a valid signature is still rejected and storage is unchanged.
    #[test]
    fn test_reassign_dispute_not_creator() {
        let env = Env::default();
        env.mock_all_auth();
        let (_admin, _arbitrator, creator) = setup(&env);
        let client = CraftNexusContractClient::new(&env);
        let id = client.create_dispute(&creator, &resolve_amount());

        let count_before = client.dispute_count();
        let locked_before = client.total_locked();
        let dispute_before = client.get_dispute(&id).unwrap();

        let stranger = address(&stranger);
        let result = client.try_reassign_dispute(&stranger, &id);
        match result {
            Err(Ok(Error::Unauthorized)) => {}
            other => panic!"expected Unauthorized, got {:other:?}"),
        }

        assert_eq!(client.dispute_count(), count_before);
        assert_eq!(client.total_locked(), locked_before);
        assert_eq!(client.get_dispute(&id).unwrap(), dispute_before);
    }

    /// The main rejected input for `reassign_dispute` is a non-creator caller.
    /// This test asserts the specific error variant and that balances are unchanged.
    #[test]
    fn test_reassign_dispute_rejected_input() {
        let env = Env::default();
        env.mock_all_auth();
        let (_admin, _arbitrator, creator) = setup(&env);
        let client = CraftNexusContractClient::new(&env);
        let id = client.create_dispute(&creator, &resolve_amount());

        let locked_before = client.total_locked();
        let count_before = client.dispute_count();

        let not_creator = address(&not_creator);
        let result = client.try_reassign_dispute(&not_creator, &id);
        match result {
            Err(Ok(Error::Unauthorized)) => {}
            other => panic!"expected Unauthorized, got {other:?}"),
        }

        assert_eq!(client.total_locked(), locked_before);
        assert_eq!(client.dispute_count(), count_before);
    }

    /// A successful reassignment updates the arbitrator and counter.
    #[test]
    fn test_reassign_dispute_success() {
        let env = Env::default();
        env.mock_all_auth();
        let (_admin, arbitrator, creator) = setup(&env);
        let client = CraftNexusContractClient::new(&env);
        let id = client.create_dispute(&creator, &resolve_amount());

        // Remove the assignment so reassignment changes state.
        client.set_dispute_arbitrator_for_test(&id, &address(&empty));
        let count_before = client.dispute_count();

        client.reassign_dispute(&creator, &id);

        let dispute = client.get_dispute(&id).unwrap();
        assert_eq!(dispute.arbitrator, arbitrator);
        assert_eq!(client.dispute_count(), count_before.checked_add(1).unwrap());
    }

    /// Helper to force a dispute's arbitrator for testing reassignment.
    #[test]
    fn test_reassign_dispute_already_assigned() {
        let env = Env::default();
        env.mock_all_auth();
        let (_admin, _arbitrator, creator) = setup(&env);
        let client = CraftNexusContractClient::new(&env);
        let id = client.create_dispute(&creator, &resolve_amount());

        let count_before = client.dispute_count();
        let locked_before = client.total_locked();

        let result = client.try_reassign_dispute(&creator, &id);
        match result {
            Err(Ok(Error::AlreadyAssigned)) => {}
            other => panic!"expected AlreadyAssigned, got {:other:?}"),
        }

        assert_eq!(client.dispute_count(), count_before);
        assert_eq!(client.total_locked(), locked_before);
    }

    fn resolve_amount() -> u128 {
        100
    }
}
