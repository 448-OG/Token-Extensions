use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    signature::Keypair, signer::Signer, system_instruction, transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address_with_program_id, instruction::create_associated_token_account,
};
use spl_token_2022::{
    extension::{memo_transfer::instruction::enable_required_transfer_memos, ExtensionType},
    instruction::{initialize_mint, reallocate},
    state::Mint,
};

fn main() {
    let mint_authority = Keypair::new();
    let mint_account = Keypair::new();

    println!("MINT ACCOUNT: {}", mint_account.pubkey());
    let decimals = 2u8;

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

    let initialize_mint_instr = initialize_mint(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        Some(&mint_authority.pubkey()),
        decimals,
    )
    .unwrap();

    let mint_authority_ata = get_associated_token_address_with_program_id(
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    println!("AUTHORITY ATA: {}", mint_authority_ata);

    let mint_authority_instr = create_associated_token_account(
        &mint_authority.pubkey(),
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );
    check_request_airdrop(&client, &mint_authority.pubkey(), 2);

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[
            create_account_instr,
            initialize_mint_instr,
            mint_authority_instr,
        ],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner_and_commitment(
            &tx,
            CommitmentConfig::finalized(),
        )
        .unwrap();

    let destination = Keypair::new();

    let destination_ata = get_associated_token_address_with_program_id(
        &destination.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    println!("DESTINATION: {}", destination.pubkey());
    println!("DESTINATION ATA: {}", destination_ata);

    let destination_ata_instr = create_associated_token_account(
        &destination.pubkey(),
        &destination.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );
    check_request_airdrop(&client, &destination.pubkey(), 2);

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[destination_ata_instr],
        Some(&destination.pubkey()),
        &[&destination],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner_and_commitment(
            &tx,
            CommitmentConfig::finalized(),
        )
        .unwrap();

    let extensions = [ExtensionType::MemoTransfer];
    let realloc_instr = reallocate(
        &spl_token_2022::id(),
        &destination_ata,
        &destination.pubkey(),
        &destination.pubkey(),
        &[&destination.pubkey()],
        &extensions,
    )
    .unwrap();
    let enable_required_memo_instr = enable_required_transfer_memos(
        &spl_token_2022::id(),
        &destination_ata,
        &destination.pubkey(),
        &[&destination.pubkey()],
    )
    .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[realloc_instr, enable_required_memo_instr],
        Some(&destination.pubkey()),
        &[&destination],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner_and_commitment(
            &tx,
            CommitmentConfig::finalized(),
        )
        .unwrap();
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
