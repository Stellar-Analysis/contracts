# Events Reference

This is the hub document for every on-chain event emitted by the contracts in
this workspace, and what (if anything) currently consumes them. Per-crate
detail lives in `docs/events/<crate>.md`; this file is the map.

## Summary

| Crate | Emits events? | Consumed by |
|---|---|---|
| `stellar_analysis` | Yes | Soroban Dashboard (snapshot feed, New Deployments panel, status panel) |
| `analytics` | Yes | Soroban Dashboard (snapshot feed, status panel, admin/governance audit trail) |
| `access-control` | Yes | New Deployments panel, audit trail |
| `escrow` | Yes | Top Contracts ranking, escrow activity feed, status panel |
| `governance` | Yes | Governance insights, New Deployments panel, audit trail |
| `governance-voting` | Storage-only today | Turnout insights (planned) |
| `multi-sig-wallet` | Storage-only today | Wallet activity feed (planned) |
| `time-locked-transactions` | Storage-only today | Status panel (planned) |
| `token-swap` | Storage-only today | Top Movers ranking (planned) |
| `upgrade` | Yes | New Deployments panel (distinguishes fresh deploy from governance upgrade) |

## `stellar_analysis`

See [`docs/events/stellar_analysis.md`](events/stellar_analysis.md).

## `analytics`

See [`docs/events/analytics.md`](events/analytics.md).

## `access-control`

See [`docs/events/access-control.md`](events/access-control.md).

## `escrow`

See [`docs/events/escrow.md`](events/escrow.md).

## `governance`

See [`docs/events/governance.md`](events/governance.md).

## `upgrade`

See [`docs/events/upgrade.md`](events/upgrade.md).
