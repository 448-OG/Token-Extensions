use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_instruction, transaction::Transaction,
};

use spl_associated_token_account::{
    get_associated_token_address, get_associated_token_address_with_program_id,
    instruction::create_associated_token_account,
};
use spl_token_2022::{
    extension::ExtensionType,
    instruction::{
        burn_checked, close_account, initialize_mint, initialize_non_transferable_mint, mint_to,
        transfer_checked,
    },
    state::Mint,
};

fn main() {
    let mint_authority = Keypair::new();
    let mint_account = Keypair::new();

    let decimals = 2u8;

    println!("AUTHORITY: {}", mint_authority.pubkey());
    println!("MINT: {}", mint_account.pubkey());
    println!("TOKEN DECIMALS: {}", decimals);

    let client = RpcClient::new("http://localhost:8899".to_string());

    check_request_airdrop(&client, &mint_authority.pubkey(), 2);

    let extensions = [ExtensionType::NonTransferable];
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
    let non_transferable_instr =
        initialize_non_transferable_mint(&spl_token_2022::id(), &mint_account.pubkey()).unwrap();
    let init_mint_instr = initialize_mint(
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
    println!("AUTHORITY  ATA: {}", mint_authority_ata);

    let mint_authority_ata_instr = create_associated_token_account(
        &mint_authority.pubkey(),
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    let mint_to_instr = mint_to(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        10,
    )
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[
            create_account_instr,
            non_transferable_instr,
            init_mint_instr,
            mint_authority_ata_instr,
            mint_to_instr,
        ],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    let destination_account = Keypair::new();
    let destination_ata =
        get_associated_token_address(&destination_account.pubkey(), &mint_account.pubkey());

    println!("DESTINATION : {}", destination_account.pubkey());
    println!("DESTINATION  ATA: {}", destination_ata);

    let destination_ata_instr = create_associated_token_account(
        &destination_account.pubkey(),
        &destination_account.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    check_request_airdrop(&client, &destination_account.pubkey(), 2);

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[destination_ata_instr],
        Some(&destination_account.pubkey()),
        &[&destination_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    let transfer_instr = transfer_checked(
        &spl_token_2022::id(),
        &mint_authority_ata,
        &mint_account.pubkey(),
        &destination_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        1,
        decimals,
    )
    .unwrap();
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[transfer_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    println!(
        "{:?}",
        client.send_and_confirm_transaction_with_spinner(&tx)
    );

    let burn_instr = burn_checked(
        &spl_token_2022::id(),
        &mint_authority_ata,
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        10,
        decimals,
    )
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[burn_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    println!(
        "{:?}",
        client.send_and_confirm_transaction_with_spinner(&tx)
    );

    //The Token Account can then be closed to recover the SOL that was allocated to the account.
    //Note that the token balance must be 0.
    let close_instr = close_account(
        &spl_token_2022::id(),
        &mint_authority_ata,
        &mint_authority.pubkey(),
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
    )
    .unwrap();
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[close_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    println!(
        "{:?}",
        client.send_and_confirm_transaction_with_spinner(&tx)
    );
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
