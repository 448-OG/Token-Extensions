use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_instruction, transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address_with_program_id, instruction::create_associated_token_account,
};
use spl_token_2022::{
    extension::{
        default_account_state::instruction::{
            initialize_default_account_state, update_default_account_state,
        },
        ExtensionType,
    },
    instruction::{initialize_mint, mint_to, thaw_account},
    state::{AccountState, Mint},
};

fn main() {
    let mint_authority = Keypair::new();
    let mint_account = Keypair::new();
    let recipient = Keypair::new();

    let decimals = 0;

    println!("AUTHORITY: {}", mint_authority.pubkey());
    println!("MINT: {}", mint_account.pubkey());
    println!("TOKEN DECIMALS: {}", decimals);

    let client = RpcClient::new("http://localhost:8899".to_string());

    check_request_airdrop(&client, &mint_authority.pubkey(), 2);

    let extensions = [ExtensionType::DefaultAccountState];
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

    let default_state = AccountState::Frozen;
    let default_state_instr = initialize_default_account_state(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &default_state,
    )
    .unwrap();
    let init_mint_instr = initialize_mint(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        Some(&mint_authority.pubkey()),
        decimals,
    )
    .unwrap();
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[create_account_instr, default_state_instr, init_mint_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    let recipient_ata = get_associated_token_address_with_program_id(
        &recipient.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    println!("RECIPIENT : {}", recipient.pubkey());
    println!("RECIPIENT ATA: {}", recipient_ata);

    let recipient_ata_instr = create_associated_token_account(
        &recipient.pubkey(),
        &recipient.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[recipient_ata_instr],
        Some(&recipient.pubkey()),
        &[&recipient],
        recent_blockhash,
    );
    check_request_airdrop(&client, &recipient.pubkey(), 2);

    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    let thaw_instr = thaw_account(
        &spl_token_2022::id(),
        &recipient_ata,
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
    )
    .unwrap();

    let mint_to_instr = mint_to(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &recipient_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        4,
    )
    .unwrap();

    let update_account_state_instr = update_default_account_state(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        &AccountState::Initialized,
    )
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[thaw_instr, mint_to_instr, update_account_state_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
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
