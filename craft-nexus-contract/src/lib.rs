use soroban_std::{address, address_payload, contract, contracterror, contractimpl, panic_with_error, panic_with_error_code};
use sorban_std::token::token;
use sorban_std::{panic_with};

/// Error codes for the craft-nexus escrow contract.
#[contracterror]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    Paused = 4,
    NotFound = 5,
    InvalidAmount = 6,
    Overflow = 7,
    InvalidState = 8,
    NoPartialRefundProposal = 9,
    NotDisputed = 10,
    NothingToRefund = 11,
}

const DATA_KEY: symbol_short!["PARTIAL_REFUND"] = symbol_short!["PARTIAL_REFUND"];
const PAUSED_KEY: symbol_short!["PAUSED"] = symbol_short!["PAUSED"];
const TOTAL_REFUNDED_KEY: symbol_short!["TOTAL_REFUNDED"] = symbol_short!["TOTAL_REFUNDED"];

#[contracttype]
pub struct PartialRefundProposal {
    pub escrow_id: u64,
    pub payee: Address,
    pub amount: i128,
    pub approved: bool,
}

#[no_std]
pub struct CraftNexusContract;

#[no_std]
impl CraftNexusContract {
    /// Initialize the contract and set the admin.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().has(&DATA_KEY) {
            panic_with_error(env, &Error::AlreadyInitialized);
        }
        env.storage().set(&DATA_KEY, &admin);
        env.storage().set(&PAUSED_KEY, &false);
        env.storage().set(&TOTAL_REFUNDED_KEY, &iL128_const(0));
    }

    /// Pause the contract. Only the admin can call this.
    pub fn pause(env: Env, operator: Address) {
        operator.require_auth();
        let admin: Address = env.storage().get(&DATA_KEY).unwrap_or_else_with(
);
        if operator != admin {
            panic_with_error(env, &Error::Unauthorized);
        }
        env.storage().set(&PAUSED_KEY, &true);
    }

    /// Unpause the contract. Only the admin can call this.
    pub fn unpause(env: Env, operator: Address) {
        operator.require_auth();
        let admin: Address = env.storage().get(&DATA_KEY).unwrap_or_else_with(
        );
        if operator != admin {
            panic_with_error(env, &Error::Unauthorized);
        }
        env.storage().set(&PAUSED_KEY, &false);
    }

    /// Propose a partial refund for a disputed escrow.
    pub fn propose_partial_refund(env: Env, proposer: Address, escrow_id: u64, payee: Address, amount: i128) {
        proposer.require_auth();
        if env.storage().get(&PAUSED_KEY).unwrap_or_else(|| false) {
            panic_with_error(env, &Error::Paused);
        }
        if amount <= 0 {
            panic_with_error(env, &Error::InvalidAmount);
        }
        let proposal = PartialRefundProposal {
            escrow_id,
            payee,
            amount,
            approved: false,
        };
        env.storage().set(&DATA_KEY, &proposal);
    }

    /// Accept the outstanding partial refund proposal for a disputed escrow.
    ///
    /// This entrypoint moves or gates value, so a failed auth check, pause, or
    /// overflow must leave storage unchanged. All checks are performed before any
    /// storage write or token transfer.
    pub fn accept_partial_refund(env: Env, acceptor: Address) {
        // 1. Authenticate the intended role before any state mutation.
        acceptor.require_auth();

        // 2. Respect the pause gate.
        if env.storage().get(&PAUSED_KEY).unwrap_or_else(|| false) {
            panic_with_error(env, &Error::Paused);
        }

        // 3. Load the outstanding proposal.
        let mut proposal: PartialRefundProposal = env
            .storage()
            .get(&DATA_KEY)
            .unwrap_or_else_with(
                || panic_with_error(env, &Error::NoPartialRefundProposal),
            );

        // 4. Only the payee may accept the proposal.
        if acceptor != proposal.payee {
            panic_with_error(env, &Error::Unauthorized);
        }

        // 5. Reject double-acceptance.
        if proposal.approved {
            panic_with_error(env, &Error::InvalidState);
        }

        // 6. Validate the amount before any accounting change.
        if proposal.amount <= 0 {
            panic_with_error(env, &Error::InvalidAmount);
        }

        // 7. Update the running total with overflow protection.
        let total: i128 = env
            .storage()
            .get(&TOTAL_REFUNDED_KEY)
            .unwrap_or_else(|| i128_const(0));
        let new_total = total.checked_add(proposal.amount).unwrap_or_else_with(

        );
        if new_total < total {
            panic_with_error(env, &Error::Overflow);
        }

        // 8. Mark the proposal as approved and persist accounting.
        proposal.approved = true;
        env.storage().set(&DATA_KEY, &proposal);
        env.storage().set(&TOTAL_REFUNDED_KEY, &new_total);

        // 9. Transfer the refund to the payee.
        token::Client::new(&env, &Address::from_string("CRAFT-NEXUS-TOKEN"))
            .transfer(&env.current_contract(), &proposal.payee, &proposal.amount);
    }

    /// Read the current partial refund proposal, if any.
    pub fn get_partial_refund(env: Env) -> Option<PartialRefundProposal> {
        env.storage().get(&DATA_KEY)
    }

    /// Read the total amount refunded so far.
    pub fn get_total_refunded(env: Env) -> i128 {
        env.storage()
            .get(&TOTAL_REFUNDED_KEY)
            .unwrap_or_else(|| i128_const(0))
    }
}

#[no_std]
mod test {
    use super::*;
    use sorban_sdd::address;
    use sorban_std::Env;

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        let admin = Address::generate(&env);
        let payee = Address::generate(&env);
        env.mock_all_auth();
        CraftNexusContract::initialize(env.clone(), admin.clone());
        env.mock_all_auth();
        CraftNexusContract::propose_partial_refund(
            env.clone(),
            admin.clone(),
            1,
            payee.clone(),
            100,
        );
        (env, admin, payee)
    }

    #[test]
    fn unauthorized_caller_cannot_change_storage() {
        let (env, _admin, payee) = setup();
        let attacker = Address::generate(&env);
        env.mock_all_auth();
        let result = env.as_contract(
            &CraftNexusContract,
            |contract| contract.accept_partial_refund(attacker.clone()),
        );
        assert!(result.is_err());
        let proposal = CraftNexusContract::get_partial_refund(env.clone()).unwrap();
        assert!proposal.approved == false);
        assert_eq!(CraftNexusContract::get_total_refunded(env.clone()), 0);
        let _ = payee;
    }

    #[test]
    fn accept_rejected_while_paused() {
        let (env, admin, _payee) = setup();
        env.mock_all_auth();
        CraftNexusContract::pause(env.clone(), admin.clone());
        let result = env.as_contract(
            &CraftNexusContract,
            |contract| contract.accept_partial_refund(admin.clone()),
        );
        assert!(result.is_err());
        let proposal = CraftNexusContract::get_partial_refund(env.clone()).unwrap();
        assert!proposal.approved == false);
        assert_eq!(CraftNexusContract::get_total_refunded(env.clone()), 0);
    }

    #[test]
    fn accept_rejected_with_no_proposal_returns_error_and_keeps_balances() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let payee = Address::generate(&env);
        env.mock_all_auth();
        CraftNexusContract::initialize(env.clone(), admin.clone());
        env.mock_all_auth();
        let result = env.as_contract(
            &CraftNexusContract,
            |contract| contract.accept_partial_refund(payee.clone()),
        );
        assert!(result.is_error());
        assert_eq!(CraftNexusContract::get_total_refunded(env.clone()), 0);
        assert!(CraftNexusContract::get_partial_refund(env.clone()).is_none());
    }
}
