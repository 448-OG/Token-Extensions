use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_instruction, transaction::Transaction,
};

use spl_token_2022::{
    extension::{
        interest_bearing_mint::{self, InterestBearingConfig},
        BaseStateWithExtensions, ExtensionType, StateWithExtensions,
    },
    instruction::initialize_mint,
    state::Mint,
};

fn main() {
    let mint_authority = Keypair::new();
    let mint_account = Keypair::new();

    // Interest rate basis points (100 = 1%) with max value of 32_767u16 (327%);
    let interest_rate = 32_767i16;

    let decimals = 2u8;

    println!("AUTHORITY: {}", mint_authority.pubkey());
    println!("MINT: {}", mint_account.pubkey());
    println!("TOKEN DECIMALS: {}", decimals);

    let client = RpcClient::new("http://localhost:8899".to_string());

    check_request_airdrop(&client, &mint_authority.pubkey(), 2);

    let extensions = [ExtensionType::InterestBearingConfig];
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
    let interest_bearing_instr = interest_bearing_mint::instruction::initialize(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        Some(mint_authority.pubkey()),
        interest_rate,
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
    let update_interest_rate_instr = interest_bearing_mint::instruction::update_rate(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        0i16,
    )
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[
            create_account_instr,
            interest_bearing_instr,
            init_mint_instr,
            update_interest_rate_instr,
        ],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    let mint = client.get_account(&mint_account.pubkey()).unwrap();
    let parsed_mint = StateWithExtensions::<Mint>::unpack(&mint.data).unwrap();
    let interest_data = parsed_mint
        .get_extension::<InterestBearingConfig>()
        .unwrap();
    let initialized_at: i64 = interest_data.initialization_timestamp.into();
    let last_updated: i64 = interest_data.last_update_timestamp.into();
    let initialized_basis_points: i16 = interest_data.pre_update_average_rate.into();
    let current_basis_points: i16 = interest_data.current_rate.into();
    println!(
        "INTEREST RATE AUTHORITY: {:?}",
        interest_data.rate_authority
    );
    println!("INITIALIZED AT: {:?}", initialized_at);
    println!("LAST UPDATED AT: {:?}", last_updated);
    println!("INITIALIZED BASIS POINTS: {:?}", initialized_basis_points);
    println!("CURRENT BASIS POINTS: {:?}", current_basis_points);
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
