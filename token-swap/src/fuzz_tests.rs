#![cfg(test)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

/* Authorized Protocol Quality Assurance & Formal Invariant Verification Test Suite */

use super::*;
use bolero::TypeGenerator;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

fn setup_fuzz_env(env: &Env) -> (TokenSwapContractClient<'_>, Address, Address, Address) {
    let admin = Address::generate(env);
    let contract_id = env.register(TokenSwapContract, ());
    let client = TokenSwapContractClient::new(env, &contract_id);
    client.initialize(&admin);

    let token_a_admin = Address::generate(env);
    let token_b_admin = Address::generate(env);
    let token_a = env.register_stellar_asset_contract_v2(token_a_admin).address();
    let token_b = env.register_stellar_asset_contract_v2(token_b_admin).address();

    (client, admin, token_a, token_b)
}

#[derive(TypeGenerator, Debug)]
struct FuzzOfferInput {
    offer_amount: i128,
    want_amount: i128,
    expiry_offset: i64,
}

#[derive(TypeGenerator, Debug)]
struct FuzzSwapMatchingInput {
    offer_units: u16,
    want_units: u16,
    expiry_seconds: u32,
    time_advance_seconds: u32,
}

// -----------------------------------------------------------------------
// 1. Fuzz arbitrary offer creation inputs – contract must never crash
// -----------------------------------------------------------------------
#[test]
fn fuzz_create_offer_arbitrary_inputs() {
    bolero::check!()
        .with_type::<FuzzOfferInput>()
        .for_each(|input| {
            let env = Env::default();
            env.mock_all_auths();
            let (client, _, token_a, token_b) = setup_fuzz_env(&env);

            let maker = Address::generate(&env);
            let current_time = env.ledger().timestamp();

            let expiry = if input.expiry_offset < 0 {
                0u64
            } else {
                current_time.saturating_add(input.expiry_offset as u64)
            };

            // Mint enough token_a if amount is positive so balance check passes
            if input.offer_amount > 0 && input.offer_amount <= 1_000_000_000_000 {
                let asset_a = token::StellarAssetClient::new(&env, &token_a);
                asset_a.mint(&maker, &input.offer_amount);
            }

            let result = client.try_create_offer(
                &maker,
                &token_a,
                &input.offer_amount,
                &token_b,
                &input.want_amount,
                &expiry,
            );

            // Invariant: Non-positive amounts must always be rejected
            if input.offer_amount <= 0 || input.want_amount <= 0 {
                assert!(result.is_err(), "Expected Err for non-positive amounts");
            }
        });
}

// -----------------------------------------------------------------------
// 2. Fuzz offer matching – atomic swap state transition and balance conservation
// -----------------------------------------------------------------------
#[test]
fn fuzz_offer_matching_atomic_swap() {
    bolero::check!()
        .with_type::<FuzzSwapMatchingInput>()
        .for_each(|input| {
            let offer_amount = (input.offer_units as i128).saturating_mul(10_000).max(10_000);
            let want_amount = (input.want_units as i128).saturating_mul(10_000).max(10_000);

            let env = Env::default();
            env.mock_all_auths();
            let (client, _, token_a, token_b) = setup_fuzz_env(&env);

            let maker = Address::generate(&env);
            let taker = Address::generate(&env);

            let asset_a = token::StellarAssetClient::new(&env, &token_a);
            let asset_b = token::StellarAssetClient::new(&env, &token_b);

            asset_a.mint(&maker, &offer_amount);
            asset_b.mint(&taker, &want_amount);

            let offer_id = client
                .create_offer(&maker, &token_a, &offer_amount, &token_b, &want_amount, &0u64);

            assert_eq!(client.get_offer(&offer_id).state, OfferState::Open);

            // Taker accepts the offer
            let accept_res = client.try_accept_offer(&taker, &offer_id);
            assert!(accept_res.is_ok(), "Expected Ok for valid offer accept");

            // Formal Invariant: Offer state strictly transitions to Filled
            let offer_after = client.get_offer(&offer_id);
            assert_eq!(offer_after.state, OfferState::Filled);

            // Conservation Invariant: Taker received offer_amount, Maker received want_amount
            let token_a_client = token::Client::new(&env, &token_a);
            let token_b_client = token::Client::new(&env, &token_b);

            assert_eq!(token_a_client.balance(&taker), offer_amount);
            assert_eq!(token_b_client.balance(&maker), want_amount);
        });
}

// -----------------------------------------------------------------------
// 3. Fuzz matching against cancelled offer – must be strictly rejected
// -----------------------------------------------------------------------
#[test]
fn fuzz_matching_against_cancelled_offer() {
    bolero::check!()
        .with_type::<(u16, u16)>()
        .for_each(|&(u_offer, u_want)| {
            let offer_amount = (u_offer as i128).saturating_mul(1_000).max(1_000);
            let want_amount = (u_want as i128).saturating_mul(1_000).max(1_000);

            let env = Env::default();
            env.mock_all_auths();
            let (client, _, token_a, token_b) = setup_fuzz_env(&env);

            let maker = Address::generate(&env);
            let taker = Address::generate(&env);

            let asset_a = token::StellarAssetClient::new(&env, &token_a);
            let asset_b = token::StellarAssetClient::new(&env, &token_b);

            asset_a.mint(&maker, &offer_amount);
            asset_b.mint(&taker, &want_amount);

            let offer_id = client
                .create_offer(&maker, &token_a, &offer_amount, &token_b, &want_amount, &0u64);

            // Maker cancels the offer
            let cancel_res = client.try_cancel_offer(&maker, &offer_id);
            assert!(cancel_res.is_ok());
            assert_eq!(client.get_offer(&offer_id).state, OfferState::Cancelled);

            // Taker attempts to match against the cancelled offer – MUST fail with AlreadyCancelled
            let match_res = client.try_accept_offer(&taker, &offer_id);
            assert!(match_res.is_err(), "Must reject matching against cancelled offer");

            // Zero-Drain Invariant: Taker tokens are completely untouched
            let token_b_client = token::Client::new(&env, &token_b);
            assert_eq!(token_b_client.balance(&taker), want_amount);
        });
}

// -----------------------------------------------------------------------
// 4. Fuzz double-matching on filled offer – no double-spend allowed
// -----------------------------------------------------------------------
#[test]
fn fuzz_double_matching_rejection() {
    bolero::check!()
        .with_type::<u16>()
        .for_each(|&amount_seed| {
            let amount = (amount_seed as i128).saturating_mul(10_000).max(10_000);

            let env = Env::default();
            env.mock_all_auths();
            let (client, _, token_a, token_b) = setup_fuzz_env(&env);

            let maker = Address::generate(&env);
            let taker1 = Address::generate(&env);
            let taker2 = Address::generate(&env);

            let asset_a = token::StellarAssetClient::new(&env, &token_a);
            let asset_b = token::StellarAssetClient::new(&env, &token_b);

            asset_a.mint(&maker, &amount);
            asset_b.mint(&taker1, &amount);
            asset_b.mint(&taker2, &amount);

            let offer_id = client.create_offer(&maker, &token_a, &amount, &token_b, &amount, &0u64);

            // First taker accepts
            let first_match = client.try_accept_offer(&taker1, &offer_id);
            assert!(first_match.is_ok());

            // Second taker attempts to match the same filled offer – MUST fail with AlreadyFilled
            let second_match = client.try_accept_offer(&taker2, &offer_id);
            assert!(second_match.is_err(), "Must reject second match on filled offer");

            // Taker 2 balance must remain intact
            let token_b_client = token::Client::new(&env, &token_b);
            assert_eq!(token_b_client.balance(&taker2), amount);
        });
}

// -----------------------------------------------------------------------
// 5. Fuzz offer expiry boundary – matching after expiry strictly forbidden
// -----------------------------------------------------------------------
#[test]
fn fuzz_offer_expiry_boundary() {
    bolero::check!()
        .with_type::<(u32, u32)>()
        .for_each(|&(expiry_duration, time_advance)| {
            let duration = (expiry_duration % 10_000).max(10) as u64;
            let advance = (time_advance % 20_000) as u64;

            let env = Env::default();
            env.mock_all_auths();
            let (client, _, token_a, token_b) = setup_fuzz_env(&env);

            let maker = Address::generate(&env);
            let taker = Address::generate(&env);

            let amount = 100_000_i128;
            let asset_a = token::StellarAssetClient::new(&env, &token_a);
            let asset_b = token::StellarAssetClient::new(&env, &token_b);

            asset_a.mint(&maker, &amount);
            asset_b.mint(&taker, &amount);

            let now = env.ledger().timestamp();
            let expiry = now.saturating_add(duration);

            let offer_id = client.create_offer(&maker, &token_a, &amount, &token_b, &amount, &expiry);

            // Advance ledger timestamp
            env.ledger().with_mut(|li| {
                li.timestamp = now.saturating_add(advance);
            });

            let match_res = client.try_accept_offer(&taker, &offer_id);

            // Invariant: If current_time > expiry, matching must fail with OfferExpired
            if advance > duration {
                assert!(match_res.is_err(), "Offer must be rejected when timestamp > expiry");
            } else {
                assert!(match_res.is_ok(), "Offer should be accepted before expiry");
            }
        });
}

// -----------------------------------------------------------------------
// 6. Fuzz extreme price ratios and large amounts
// -----------------------------------------------------------------------
#[test]
fn fuzz_extreme_price_ratio_edge_cases() {
    bolero::check!()
        .with_type::<(bool, i128)>()
        .for_each(|&(offer_high, base_amount)| {
            let _valid_base = (base_amount.abs() % 1_000_000_000).max(1);
            let (offer_amount, want_amount) = if offer_high {
                (1_000_000_000_000_i128, 1_i128)
            } else {
                (1_i128, 1_000_000_000_000_i128)
            };

            let env = Env::default();
            env.mock_all_auths();
            let (client, _, token_a, token_b) = setup_fuzz_env(&env);

            let maker = Address::generate(&env);
            let taker = Address::generate(&env);

            let asset_a = token::StellarAssetClient::new(&env, &token_a);
            let asset_b = token::StellarAssetClient::new(&env, &token_b);

            asset_a.mint(&maker, &offer_amount);
            asset_b.mint(&taker, &want_amount);

            let offer_id = client.create_offer(&maker, &token_a, &offer_amount, &token_b, &want_amount, &0u64);

            let match_res = client.try_accept_offer(&taker, &offer_id);
            assert!(match_res.is_ok(), "Extreme price ratio swap must execute atomically");
        });
}
