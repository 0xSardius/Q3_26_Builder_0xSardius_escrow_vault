# Escrow (Turbin3 Q3 2026)

A timed, trustless token swap on Solana. The maker locks token A in a program-owned vault and names a price in token B. While the offer is live, any taker can complete the swap atomically. After expiry, take and update close; whoever holds a 1-unit position receipt can unwind the vault.

The program is **stateless**. All offer data lives on a per-offer escrow PDA. Vault movement is shared helpers with no clock or policy of their own — instructions decide *when* tokens move; `withdraw` / `close` only decide *how*.

Program ID: `5Y6HMSgNYbkcBiQCukYvTK56aQarSpq1Nk9aiSsjws2o`  
Stack: Anchor 1.1.2, LiteSVM tests.

## Why this shape

A classic escrow is “maker deposits, taker fills, or maker refunds.” That is enough for a classroom swap. Two extra constraints make it a real offer, not a locker:

1. **Time.** An open offer that never expires is an unpaid option on the maker’s inventory. `Clock` splits the world into live vs expired so take cannot fill a stale book and refund cannot yank a live one.
2. **Claim.** The vault is PDA-owned, so the right to unwind it has to live *somewhere*. A 1-unit SPL mint (decimals 0) is that right. Transferring the receipt transfers the claim; burning it retires the claim. Take and refund only succeed if the maker still holds it.

That last point is the Task 5 “small vault”: not a second SOL piggy-bank, and not a full position market. It is a transferable claim on *this* escrow’s token-A ATA.

```
make  →  (update)*  →  take XOR refund XOR redeem
              live              expired (refund / redeem)
```

## Accounts

| Account | Derivation / ownership | Role |
|---|---|---|
| Escrow | PDA `["escrow", maker, seed]` | Offer state |
| Vault | ATA of `(escrow, mint_a)` | Locked token A. Authority = escrow PDA |
| Position mint | PDA `["position", escrow]` | 1-unit receipt mint. Mint authority = escrow |
| Maker position ATA | ATA of `(maker, position_mint)` | Starts at 1; take/refund require it still be 1 |

```rust
pub struct Escrow {
    pub seed: u64,
    pub maker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub receive: u64,
    pub bump: u8,
    pub expiration: i64,
    pub position_mint: Pubkey,
}
```

PDA seeds include `maker` and `seed` so one wallet can open many offers without colliding, and so the maker is bound into the address (you cannot point a foreign escrow at someone else’s maker).

## Instructions

Discriminators are explicit (`0`–`4`) so the IDL order cannot silently shift.

| Disc | Instruction | Clock | Signer | Effect |
|---|---|---|---|---|
| 0 | `make` | expiration must be `> now` | Maker | Init escrow + vault, deposit A, mint receipt, approve escrow as delegate |
| 1 | `take` | `now < expiration` | Taker | Burn receipt (via delegate) → pay B to maker → send A to taker → close vault |
| 2 | `refund` | `now >= expiration` | Maker | Burn receipt (as owner) → return A to maker → close vault |
| 3 | `update` | `now < expiration` | Maker | Change `receive` only |
| 4 | `redeem` | `now >= expiration` | Holder of the receipt | Burn receipt → send A to holder → close vault (rent to holder). Escrow `close = maker` |

`lib.rs` is the sequencer. Each `#[derive(Accounts)]` struct owns its steps (`assert_*`, `burn_position`, `deposit`, `withdraw`, `close_vault`). That keeps account validation next to the CPI that uses those accounts.

**Burn is always first** on take, refund, and redeem. A failed burn must not leave tokens moved and a receipt still outstanding.

### Clock lives on the instruction, not on withdraw

`withdraw_from_vault` and `close_vault` take a destination and signer seeds. They do not read `Clock`. Putting `assert_live` inside withdraw would make refund impossible: refund *must* move tokens after expiry.

| Error | When |
|---|---|
| `EscrowExpirationInPast` | `make` with `expiration <= now` |
| `EscrowExpired` | `take` / `update` after expiry |
| `EscrowNotExpired` | `refund` / `redeem` while live |
| `MissingPosition` | Receipt ATA amount is not 1 |

### Take vs refund vs redeem

Take is the fill. The taker is the only signer, so the maker cannot be asked to burn. On `make`, the maker `approve`s the escrow PDA as delegate for 1 receipt token; take burns with escrow seeds.

Refund is the maker’s unwind after expiry, and only if they still hold the receipt.

Redeem is the holder’s unwind after expiry. Transferring the receipt is the “exit anytime” path: you sold the claim, you did not pull the vault early. After that transfer, take and refund fail `MissingPosition`. The holder waits for expiry.

```
maker still holds receipt
  live     → take
  expired  → refund  (or redeem; same person, different destination for rent)

maker transferred receipt
  live     → take/refund fail
  expired  → holder redeem
```

## Shared vault helpers

Task 1 on this program is withdraw + close of the escrow’s token ATA, not a standalone SOL vault.

- [`withdraw.rs`](programs/escrowq32026/src/instructions/withdraw.rs) — `transfer_checked` of `vault.amount`, signed by escrow seeds
- [`close.rs`](programs/escrowq32026/src/instructions/close.rs) — close the ATA, send rent to the destination (taker, maker, or holder)

Every exit reuses both. Policy stays in `take` / `refund` / `redeem`.

## BPF stack

Solana BPF frames are 4096 bytes. Deserialized `Mint` / `TokenAccount` values are large. `Take`, `Refund`, and `Redeem` `Box` those accounts so the frame holds pointers and the structs live on the heap. Without that, adding the position accounts overflowed refund (`4104` vs `4096`) and the runtime trapped on a stack access violation.

## Tests

LiteSVM in [`programs/escrowq32026/tests/mod.rs`](programs/escrowq32026/tests/mod.rs). Clock is a sysvar, so expiry tests call `set_clock` instead of sleeping.

```
test_make_and_refund              make → warp → refund; vault gone, A restored, receipt = 0
test_update                      receive changes while live
test_take                        swap completes; receipt = 0
test_refund_rejected_while_live
test_take_rejected_when_expired
test_redeem_after_expiry
test_redeem_rejected_while_live
```

Build the `.so` before the test harness, which `include_bytes!`s the deploy artifact:

```bash
NO_DNA=1 cargo build-sbf --manifest-path programs/escrowq32026/Cargo.toml
NO_DNA=1 cargo test -p escrowq32026 --test mod
```

## Layout

```
programs/escrowq32026/src/
  lib.rs                 instruction order only
  state.rs               Escrow
  error.rs
  constants.rs           ESCROW_SEED, POSITION_SEED
  instructions/
    make.rs
    take.rs
    refund.rs
    update.rs
    redeem.rs
    withdraw.rs          shared, no policy
    close.rs             shared, no policy
```
