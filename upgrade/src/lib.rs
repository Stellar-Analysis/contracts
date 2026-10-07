//! `upgrade` contract: governance-gated contract upgrade proposals, approvals,
//! timelocks, and executions for the Stellar Analysis protocol.
//!
//! # Public API
//! - `init` — initializes governance address, initial version (1.0.0), and proposal counter
//! - `version` — returns the current `ContractVersion` (major, minor, patch)
//! - `propose_upgrade` — creates a new upgrade proposal with 48h timelock (governance-only)
//! - `approve_upgrade` — approves a pending upgrade proposal (governance-only)
//! - `execute_upgrade` — executes an approved proposal after timelock expiry and updates Wasm bytecode
//! - `emergency_upgrade` — bypasses timelock for critical fixes (governance-only)
//! - `get_proposal` — queries an `UpgradeProposal` by its ID
//!
//! # Events
//! See `docs/events/upgrade.md` and `docs/EVENTS.md`:
//! - `UpgradeApprovedEvent` (`upg_appr`, `proposal_id`) emitted on `approve_upgrade`
//! - `UpgradeExecutedEvent` (`upg_exec`, `proposal_id`) emitted on `execute_upgrade`
//!
//! # State
//! Stored in instance storage under `DataKey`:
//! - `DataKey::Governance` -> `Address`
//! - `DataKey::Version` -> `ContractVersion`
//! - `DataKey::ProposalCount` -> `u32`
//! - `DataKey::Proposal(u32)` -> `UpgradeProposal`

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeProposal {
    pub id: u32,
    pub new_wasm_hash: BytesN<32>,
    pub proposer: Address,
    pub created_at: u64,
    pub timelock_until: u64,
    pub status: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Governance,
    Version,
    ProposalCount,
    Proposal(u32),
}

/// Emitted when an upgrade proposal is approved by governance.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeApprovedEvent {
    pub proposal_id: u32,
    pub approver: Address,
}

/// Emitted when an approved upgrade proposal is executed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpgradeExecutedEvent {
    pub proposal_id: u32,
    pub new_wasm_hash: BytesN<32>,
    pub executor: Address,
}

#[contract]
pub struct UpgradeManager;

#[contractimpl]
impl UpgradeManager {
    pub fn init(env: Env, governance: Address) {
        env.storage()
            .instance()
            .set(&DataKey::Governance, &governance);
        env.storage()
            .instance()
            .set(&DataKey::Version, &ContractVersion {
                major: 1,
                minor: 0,
                patch: 0,
            });
        env.storage()
            .instance()
            .set(&DataKey::ProposalCount, &0u32);
    }

    pub fn version(env: Env) -> ContractVersion {
        env.storage()
            .instance()
            .get(&DataKey::Version)
            .unwrap_or(ContractVersion {
                major: 1,
                minor: 0,
                patch: 0,
            })
    }

    pub fn propose_upgrade(env: Env, new_wasm_hash: BytesN<32>) -> u32 {
        let governance: Address = env
            .storage()
            .instance()
            .get(&DataKey::Governance)
            .expect("Governance not initialized");

        governance.require_auth();

        let proposal_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::ProposalCount)
            .unwrap_or(0);

        let proposal_id = proposal_count + 1;
        let timelock_until = env.ledger().timestamp() + (48 * 3600);

        let proposal = UpgradeProposal {
            id: proposal_id,
            new_wasm_hash: new_wasm_hash.clone(),
            proposer: governance,
            created_at: env.ledger().timestamp(),
            timelock_until,
            status: 0,
        };

        env.storage()
            .instance()
            .set(&DataKey::ProposalCount, &proposal_id);
        env.storage()
            .instance()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        proposal_id
    }

    pub fn approve_upgrade(env: Env, proposal_id: u32) {
        let governance: Address = env
            .storage()
            .instance()
            .get(&DataKey::Governance)
            .expect("Governance not initialized");

        governance.require_auth();

        let mut proposal: UpgradeProposal = env
            .storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found");

        if proposal.status != 0 {
            panic!("Proposal is not pending");
        }

        proposal.status = 1;
        env.storage()
            .instance()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        // Emit UpgradeApprovedEvent
        env.events().publish(
            (symbol_short!("UPG_APP"),),
            (proposal_id, governance),
        );
    }

    pub fn execute_upgrade(env: Env, proposal_id: u32) {
        let mut proposal: UpgradeProposal = env
            .storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found");

        if proposal.status != 1 {
            panic!("Proposal is not approved");
        }

        if env.ledger().timestamp() < proposal.timelock_until {
            panic!("Timelock period not expired");
        }

        let old_version = Self::version(env.clone());

        env.deployer()
            .update_current_contract_wasm(proposal.new_wasm_hash.clone());

        proposal.status = 2;
        env.storage()
            .instance()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        let new_version = ContractVersion {
            major: old_version.major,
            minor: old_version.minor + 1,
            patch: 0,
        };
        env.storage()
            .instance()
            .set(&DataKey::Version, &new_version);

        let governance: Address = env
            .storage()
            .instance()
            .get(&DataKey::Governance)
            .expect("Governance not initialized");

        // Emit UpgradeExecutedEvent
        env.events().publish(
            (symbol_short!("UPG_EXE"),),
            (proposal_id, proposal.new_wasm_hash, governance),
        );
    }

    pub fn emergency_upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        let governance: Address = env
            .storage()
            .instance()
            .get(&DataKey::Governance)
            .expect("Governance not initialized");

        governance.require_auth();

        env.deployer()
            .update_current_contract_wasm(new_wasm_hash);

        let new_version = ContractVersion {
            major: 1,
            minor: 0,
            patch: 1,
        };
        env.storage()
            .instance()
            .set(&DataKey::Version, &new_version);
    }

    pub fn get_proposal(env: Env, proposal_id: u32) -> UpgradeProposal {
        env.storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .expect("Proposal not found")
    }
}

#[cfg(test)]
mod test;
