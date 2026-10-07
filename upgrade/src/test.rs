#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    symbol_short, Address, BytesN, Env, Symbol, TryFromVal,
};

fn has_event_with_topic0(env: &Env, topic0: Symbol) -> bool {
    env.events().all().events().iter().any(|e| {
        let soroban_sdk::xdr::ContractEventBody::V0(ref v0) = e.body;
        if v0.topics.is_empty() {
            return false;
        }
        <Symbol as TryFromVal<Env, soroban_sdk::xdr::ScVal>>::try_from_val(env, &v0.topics[0])
            .map(|t| t == topic0)
            .unwrap_or(false)
    })
}

#[test]
fn test_init_and_version() {
    let env = Env::default();
    env.mock_all_auths();

    let governance = Address::generate(&env);
    let contract_id = env.register(UpgradeManager, ());
    let client = UpgradeManagerClient::new(&env, &contract_id);

    client.init(&governance);

    let ver = client.version();
    assert_eq!(ver.major, 1);
    assert_eq!(ver.minor, 0);
    assert_eq!(ver.patch, 0);
}

#[test]
fn test_propose_and_approve_upgrade_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let governance = Address::generate(&env);
    let contract_id = env.register(UpgradeManager, ());
    let client = UpgradeManagerClient::new(&env, &contract_id);

    client.init(&governance);

    let wasm_hash = BytesN::from_array(&env, &[7u8; 32]);
    let proposal_id = client.propose_upgrade(&wasm_hash);
    assert_eq!(proposal_id, 1);

    let prop = client.get_proposal(&proposal_id);
    assert_eq!(prop.status, 0);
    assert_eq!(prop.proposer, governance);

    // Approve upgrade
    client.approve_upgrade(&proposal_id);

    // Assert UpgradeApprovedEvent was published
    assert!(
        has_event_with_topic0(&env, symbol_short!("UPG_APP")),
        "expected an UPG_APP event on approve_upgrade"
    );

    let prop_approved = client.get_proposal(&proposal_id);
    assert_eq!(prop_approved.status, 1);
}

#[test]
#[should_panic(expected = "Proposal is not pending")]
fn test_double_approve_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let governance = Address::generate(&env);
    let contract_id = env.register(UpgradeManager, ());
    let client = UpgradeManagerClient::new(&env, &contract_id);

    client.init(&governance);

    let wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
    let proposal_id = client.propose_upgrade(&wasm_hash);
    client.approve_upgrade(&proposal_id);
    client.approve_upgrade(&proposal_id);
}

#[test]
#[should_panic(expected = "Proposal is not approved")]
fn test_execute_unapproved_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let governance = Address::generate(&env);
    let contract_id = env.register(UpgradeManager, ());
    let client = UpgradeManagerClient::new(&env, &contract_id);

    client.init(&governance);

    let wasm_hash = BytesN::from_array(&env, &[2u8; 32]);
    let proposal_id = client.propose_upgrade(&wasm_hash);
    client.execute_upgrade(&proposal_id);
}

#[test]
#[should_panic(expected = "Timelock period not expired")]
fn test_execute_before_timelock_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let governance = Address::generate(&env);
    let contract_id = env.register(UpgradeManager, ());
    let client = UpgradeManagerClient::new(&env, &contract_id);

    client.init(&governance);

    let wasm_hash = BytesN::from_array(&env, &[3u8; 32]);
    let proposal_id = client.propose_upgrade(&wasm_hash);
    client.approve_upgrade(&proposal_id);
    client.execute_upgrade(&proposal_id);
}
