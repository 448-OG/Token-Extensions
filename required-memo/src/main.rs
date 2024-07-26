use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    signature::Keypair, signer::Signer, system_instruction, transaction::Transaction,
};
use solana_transaction_status::UiTransactionEncoding;
use spl_associated_token_account::{
    get_associated_token_address_with_program_id, instruction::create_associated_token_account,
};
use spl_token_2022::{
    extension::{
        memo_transfer::instruction::{
            disable_required_transfer_memos, enable_required_transfer_memos,
        },
        ExtensionType,
    },
    instruction::{initialize_account, initialize_mint, mint_to, transfer_checked},
    state::{Account, Mint},
};

fn main() {
    let mint_authority = Keypair::new();
    let mint_account = Keypair::new();

    println!("MINT AUTHORITY: {}", mint_authority.pubkey());
    println!("MINT ACCOUNT: {}", mint_account.pubkey());
    let decimals = 0u8;

    let client = RpcClient::new("http://localhost:8899".to_string());

    let extensions = [];
    let mint_len = ExtensionType::try_calculate_account_len::<Mint>(&extensions).unwrap();
    let rent = client
        .get_minimum_balance_for_rent_exemption(mint_len)
        .unwrap();

    let create_account_instr = system_instruction::create_account(
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        rent,
        mint_len as u64,
        &spl_token_2022::id(),
    );

    let init_mint_instr = initialize_mint(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        Some(&mint_authority.pubkey()),
        decimals,
    )
    .unwrap();

    let token_account = Keypair::new();
    println!("TOKEN ACCOUNT: {}", &token_account.pubkey());

    let token_account_ext = [ExtensionType::MemoTransfer];
    let token_account_len =
        ExtensionType::try_calculate_account_len::<Account>(&token_account_ext).unwrap();
    let token_account_rent = client
        .get_minimum_balance_for_rent_exemption(token_account_len)
        .unwrap();

    let token_account_instr = system_instruction::create_account(
        &mint_authority.pubkey(),
        &token_account.pubkey(),
        token_account_rent,
        token_account_len as u64,
        &spl_token_2022::id(),
    );
    let init_token_account_instr = initialize_account(
        &spl_token_2022::id(),
        &token_account.pubkey(),
        &mint_account.pubkey(),
        &token_account.pubkey(),
    )
    .unwrap();

    let enable_memo_instr = enable_required_transfer_memos(
        &spl_token_2022::id(),
        &token_account.pubkey(),
        &token_account.pubkey(),
        &[&mint_authority.pubkey(), &token_account.pubkey()],
    )
    .unwrap();

    check_request_airdrop(&client, &mint_authority.pubkey(), 2);

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[
            create_account_instr,
            init_mint_instr,
            token_account_instr,
            init_token_account_instr,
            enable_memo_instr,
        ],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account, &token_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner_and_commitment(
            &tx,
            CommitmentConfig::finalized(),
        )
        .unwrap();

    let source_account = create_associated_token_account(
        &mint_authority.pubkey(),
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );
    let mint_authority_ata = get_associated_token_address_with_program_id(
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );
    println!("MINT AUTHORITY ATA: {}", mint_authority_ata);
    let mint_to_instr = mint_to(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey()],
        200,
    )
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[source_account, mint_to_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner_and_commitment(
            &tx,
            CommitmentConfig::finalized(),
        )
        .unwrap();

    let transfer_instr = transfer_checked(
        &spl_token_2022::id(),
        &mint_authority_ata,
        &mint_account.pubkey(),
        &token_account.pubkey(),
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey()],
        12,
        decimals,
    )
    .unwrap();

    let memo_instr =
        spl_memo::build_memo(b"Learning Token Extensions", &[&mint_authority.pubkey()]);

    println!("Transferring Without Memo");
    //Attempt to transfer without a memo
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[transfer_instr.clone()],
        Some(&mint_authority.pubkey()),
        &[&mint_authority],
        recent_blockhash,
    );
    if let Err(error) = client.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx,
        CommitmentConfig::finalized(),
    ) {
        dbg!(&error);
    }

    println!("Transferring With Memo");
    let tx = Transaction::new_signed_with_payer(
        &[memo_instr, transfer_instr.clone()],
        Some(&mint_authority.pubkey()),
        &[&mint_authority],
        recent_blockhash,
    );
    match client.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx,
        CommitmentConfig::finalized(),
    ) {
        Err(error) => {
            dbg!(&error);
        }
        Ok(sig) => {
            println!("TRANSFERRED WITH MEMO");
            dbg!(
                client
                    .get_transaction(&sig, UiTransactionEncoding::Json)
                    .unwrap()
                    .transaction
                    .meta
                    .unwrap()
                    .log_messages
            );
        }
    }

    // A disable memo transfers instruction
    let disable_memo_instr = disable_required_transfer_memos(
        &spl_token_2022::id(),
        &token_account.pubkey(),
        &token_account.pubkey(),
        &[&mint_authority.pubkey(), &token_account.pubkey()],
    )
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[disable_memo_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &token_account],
        recent_blockhash,
    );
    if let Err(error) = client.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx,
        CommitmentConfig::finalized(),
    ) {
        dbg!(&error);
    }

    println!("Transferring Without Memo After Disabling Memo Requirement");
    //Attempt to transfer without a memo
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[transfer_instr.clone()],
        Some(&mint_authority.pubkey()),
        &[&mint_authority],
        recent_blockhash,
    );
    match client.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx,
        CommitmentConfig::finalized(),
    ) {
        Err(error) => {
            dbg!(&error);
        }
        Ok(sig) => {
            println!("TRANSFERRED WITHOUT MEMO");
            dbg!(
                client
                    .get_transaction(&sig, UiTransactionEncoding::Json)
                    .unwrap()
                    .transaction
                    .meta
                    .unwrap()
                    .log_messages
            );
        }
    }
}

fn check_request_airdrop(client: &RpcClient, account: &Pubkey, amount: u64) {
    if client.get_balance(&account).unwrap().eq(&0) {
        client
            .request_airdrop(&account, LAMPORTS_PER_SOL * amount)
            .unwrap();

        loop {
            if (LAMPORTS_PER_SOL).gt(&client.get_balance(&account).unwrap()) {
                println!("Airdrop for {} has not reflected ...", account);
                std::thread::sleep(std::time::Duration::from_secs(1));
            } else {
                println!("\nAirdrop for {} has reflected!\n", account);

                break;
            }
        }
    }
}
