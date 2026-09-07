use {
    anchor_lang::{
        prelude::msg, solana_program::instruction::Instruction, solana_program::program_pack::Pack,
        system_program::ID as SYSTEM_PROGRAM_ID, AccountDeserialize, InstructionData,
        ToAccountMetas,
    },
    anchor_spl::{
        associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
        token::spl_token,
    },
    litesvm::LiteSVM,
    litesvm_token::{
        spl_token::ID as TOKEN_PROGRAM_ID, CreateAssociatedTokenAccount, CreateMint, MintTo,
    },
    solana_clock::Clock,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

const SEED: u64 = 123;
const DEPOSIT: u64 = 10_000_000;
const RECEIVE: u64 = 10_000_000;
const EXPIRATION: i64 = 17780206209;

struct Offer {
    mint_a: Pubkey,
    mint_b: Pubkey,
    maker_ata_a: Pubkey,
    escrow: Pubkey,
    vault: Pubkey,
    position_mint: Pubkey,
    maker_position_ata: Pubkey,
}

fn setup() -> (LiteSVM, Keypair) {
    let program_id = escrowq32026::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    // build-sbf writes the .so next to target/tmp; this path is that deploy artifact.
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/escrowq32026.so"
    ));
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    (svm, payer)
}

fn send(svm: &mut LiteSVM, payer: &Keypair, extra: &[&Keypair], ix: Instruction) {
    try_send(svm, payer, extra, ix).unwrap();
}

fn try_send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    extra: &[&Keypair],
    ix: Instruction,
) -> litesvm::types::TransactionResult {
    let message = Message::new(&[ix], Some(&payer.pubkey()));
    let recent_blockhash = svm.latest_blockhash();
    let mut signers = vec![payer];
    signers.extend_from_slice(extra);
    let transaction = Transaction::new(&signers, message, recent_blockhash);
    svm.send_transaction(transaction)
}

fn set_clock(svm: &mut LiteSVM, unix_timestamp: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_timestamp;
    svm.set_sysvar(&clock);
}

fn token_amount(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let account = svm.get_account(account).unwrap();
    spl_token::state::Account::unpack(&account.data)
        .unwrap()
        .amount
}

fn escrow_data(svm: &LiteSVM, escrow: &Pubkey) -> escrowq32026::state::Escrow {
    let account = svm.get_account(escrow).unwrap();
    escrowq32026::state::Escrow::try_deserialize(&mut account.data.as_ref()).unwrap()
}

fn make_offer(svm: &mut LiteSVM, payer: &Keypair, receive: u64) -> Offer {
    let maker = payer.pubkey();

    let mint_a = CreateMint::new(svm, payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    let mint_b = CreateMint::new(svm, payer)
        .decimals(6)
        .authority(&maker)
        .send()
        .unwrap();
    let maker_ata_a = CreateAssociatedTokenAccount::new(svm, payer, &mint_a)
        .owner(&maker)
        .send()
        .unwrap();

    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &SEED.to_le_bytes()],
        &escrowq32026::id(),
    )
    .0;
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
    let position_mint = Pubkey::find_program_address(&[b"position", escrow.as_ref()], &escrowq32026::id()).0;
    let maker_position_ata =
        associated_token::get_associated_token_address(&maker, &position_mint);

    MintTo::new(svm, payer, &mint_a, &maker_ata_a, 1_000_000_000)
        .send()
        .unwrap();

    send(
        svm,
        payer,
        &[],
        Instruction {
            program_id: escrowq32026::id(),
            accounts: escrowq32026::accounts::Make {
                maker,
                mint_a,
                mint_b,
                maker_ata_a,
                escrow,
                vault,
                position_mint,
                maker_position_ata,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: escrowq32026::instruction::Make {
                deposit: DEPOSIT,
                seed: SEED,
                receive,
                expiration: EXPIRATION,
            }
            .data(),
        },
    );

    Offer {
        mint_a,
        mint_b,
        maker_ata_a,
        escrow,
        vault,
        position_mint,
        maker_position_ata,
    }
}

#[test]
fn test_make_and_refund() {
    let (mut svm, payer) = setup();
    let offer = make_offer(&mut svm, &payer, RECEIVE);

    assert_eq!(token_amount(&svm, &offer.vault), DEPOSIT);
    let escrow = escrow_data(&svm, &offer.escrow);
    assert_eq!(escrow.seed, SEED);
    assert_eq!(escrow.maker, payer.pubkey());
    assert_eq!(escrow.mint_a, offer.mint_a);
    assert_eq!(escrow.mint_b, offer.mint_b);
    assert_eq!(escrow.receive, RECEIVE);
    assert_eq!(escrow.position_mint, offer.position_mint);
    assert_eq!(token_amount(&svm, &offer.maker_position_ata), 1);

    // Refund is only legal after the offer lapses.
    set_clock(&mut svm, EXPIRATION);

    send(
        &mut svm,
        &payer,
        &[],
        Instruction {
            program_id: escrowq32026::id(),
            accounts: escrowq32026::accounts::Refund {
                maker: payer.pubkey(),
                mint_a: offer.mint_a,
                maker_ata_a: offer.maker_ata_a,
                escrow: offer.escrow,
                vault: offer.vault,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: escrowq32026::instruction::Refund {}.data(),
        },
    );

    msg!("Refund closed escrow and vault");
    assert!(svm.get_account(&offer.escrow).is_none());
    assert!(svm.get_account(&offer.vault).is_none());
    assert_eq!(token_amount(&svm, &offer.maker_ata_a), 1_000_000_000);
}

#[test]
fn test_update() {
    let (mut svm, payer) = setup();
    let offer = make_offer(&mut svm, &payer, RECEIVE);
    let new_receive = 25_000_000;

    send(
        &mut svm,
        &payer,
        &[],
        Instruction {
            program_id: escrowq32026::id(),
            accounts: escrowq32026::accounts::Update {
                maker: payer.pubkey(),
                escrow: offer.escrow,
            }
            .to_account_metas(None),
            data: escrowq32026::instruction::Update {
                receive: new_receive,
            }
            .data(),
        },
    );

    let escrow = escrow_data(&svm, &offer.escrow);
    assert_eq!(escrow.receive, new_receive);
    // Update only changes the ask; deposited mint A stays locked in the vault.
    assert_eq!(token_amount(&svm, &offer.vault), DEPOSIT);
}

#[test]
fn test_take() {
    let (mut svm, payer) = setup();
    let offer = make_offer(&mut svm, &payer, RECEIVE);
    let maker = payer.pubkey();

    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();

    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &taker, &offer.mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    // Maker is mint_b's authority, so they can mint into the taker's ATA.
    MintTo::new(&mut svm, &payer, &offer.mint_b, &taker_ata_b, RECEIVE)
        .send()
        .unwrap();

    // Addresses only — take's init_if_needed creates these ATAs if missing.
    let taker_ata_a =
        associated_token::get_associated_token_address(&taker.pubkey(), &offer.mint_a);
    let maker_ata_b = associated_token::get_associated_token_address(&maker, &offer.mint_b);

    // Taker is the only signer; the maker already locked tokens in make.
    send(
        &mut svm,
        &taker,
        &[],
        Instruction {
            program_id: escrowq32026::id(),
            accounts: escrowq32026::accounts::Take {
                taker: taker.pubkey(),
                maker,
                mint_a: offer.mint_a,
                mint_b: offer.mint_b,
                taker_ata_a,
                taker_ata_b,
                maker_ata_b,
                escrow: offer.escrow,
                vault: offer.vault,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: escrowq32026::instruction::Take {}.data(),
        },
    );

    msg!("Take swapped tokens and closed vault");
    assert!(svm.get_account(&offer.escrow).is_none());
    assert!(svm.get_account(&offer.vault).is_none());
    assert_eq!(token_amount(&svm, &taker_ata_a), DEPOSIT);
    assert_eq!(token_amount(&svm, &maker_ata_b), RECEIVE);
}

#[test]
fn test_refund_rejected_while_live() {
    let (mut svm, payer) = setup();
    let offer = make_offer(&mut svm, &payer, RECEIVE);

    let result = try_send(
        &mut svm,
        &payer,
        &[],
        Instruction {
            program_id: escrowq32026::id(),
            accounts: escrowq32026::accounts::Refund {
                maker: payer.pubkey(),
                mint_a: offer.mint_a,
                maker_ata_a: offer.maker_ata_a,
                escrow: offer.escrow,
                vault: offer.vault,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: escrowq32026::instruction::Refund {}.data(),
        },
    );

    assert!(result.is_err());
    assert!(svm.get_account(&offer.escrow).is_some());
}

#[test]
fn test_take_rejected_when_expired() {
    let (mut svm, payer) = setup();
    let offer = make_offer(&mut svm, &payer, RECEIVE);
    let maker = payer.pubkey();

    let taker = Keypair::new();
    svm.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();
    let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &taker, &offer.mint_b)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    MintTo::new(&mut svm, &payer, &offer.mint_b, &taker_ata_b, RECEIVE)
        .send()
        .unwrap();

    set_clock(&mut svm, EXPIRATION);

    let result = try_send(
        &mut svm,
        &taker,
        &[],
        Instruction {
            program_id: escrowq32026::id(),
            accounts: escrowq32026::accounts::Take {
                taker: taker.pubkey(),
                maker,
                mint_a: offer.mint_a,
                mint_b: offer.mint_b,
                taker_ata_a: associated_token::get_associated_token_address(
                    &taker.pubkey(),
                    &offer.mint_a,
                ),
                taker_ata_b,
                maker_ata_b: associated_token::get_associated_token_address(&maker, &offer.mint_b),
                escrow: offer.escrow,
                vault: offer.vault,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: escrowq32026::instruction::Take {}.data(),
        },
    );

    assert!(result.is_err());
    assert!(svm.get_account(&offer.escrow).is_some());
}
