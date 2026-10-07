# `upgrade` — Event Reference

`upgrade/src/lib.rs` emits 2 on-chain events covering the contract upgrade lifecycle.

| Event | Topic(s) | Trigger | Consumer |
|---|---|---|---|
| `UpgradeApprovedEvent` | `(symbol_short!("UPG_APP"),)` | Every successful `approve_upgrade` call (governance-only, after proposal validation) | New Deployments panel / governance audit trail |
| `UpgradeExecutedEvent` | `(symbol_short!("UPG_EXE"),)` | Every successful `execute_upgrade` call (after timelock expiration and contract Wasm bytecode update) | New Deployments panel — distinguishes fresh deploys from governance-approved upgrades |

## Field reference

**`UpgradeApprovedEvent`**:
- `proposal_id`: `u32` identifier of the approved upgrade proposal
- `approver`: `Address` of the governance caller that authorized the upgrade

**`UpgradeExecutedEvent`**:
- `proposal_id`: `u32` identifier of the executed proposal
- `new_wasm_hash`: `BytesN<32>` hash of the newly installed contract Wasm bytecode
- `executor`: `Address` of the governance caller that executed the upgrade

See `docs/EVENTS.md` for the cross-crate summary.
