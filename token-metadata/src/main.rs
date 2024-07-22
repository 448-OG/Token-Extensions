use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, message::Message, native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};
use spl_token_2022::{extension::ExtensionType, state::Mint};
use spl_token_metadata_interface::state::TokenMetadata;
use spl_type_length_value::variable_len_pack::VariableLenPack;

pub const INIT_BEFORE_UPDATE: &str = "Initializing...";

fn main() {
    let mint_authority = Keypair::new();
    let mint_account = Keypair::new();
    println!("MINT ACCOUNT: {}", mint_account.pubkey());

    let client = RpcClient::new("http://localhost:8899".to_string());

    let name = "FOO-CLUB";
    let symbol = "FFF";
    let uri = "http://example.com";

    let mut metadata = TokenMetadata {
        mint: mint_account.pubkey(),
        name: name.into(),
        symbol: symbol.into(),
        uri: uri.into(),
        additional_metadata: vec![("membership".to_string(), INIT_BEFORE_UPDATE.into())],
        ..Default::default()
    };
    metadata.update_authority.0 = mint_authority.pubkey();

    // Size of MetadataExtension 2 bytes for type, 2 bytes for length
    let metadata_extension_len = 4usize;
    let metadata_extension_bytes_len = metadata.get_packed_len().unwrap();
    let extensions = vec![ExtensionType::MetadataPointer];
    let mint_len = ExtensionType::try_calculate_account_len::<Mint>(&extensions).unwrap();
    let mut rent_for_extensions = client
        .get_minimum_balance_for_rent_exemption(
            mint_len + metadata_extension_len + metadata_extension_bytes_len,
        )
        .unwrap();
    // Ensure enough space can be allocated for the additional info
    rent_for_extensions += rent_for_extensions - INIT_BEFORE_UPDATE.len() as u64;

    let create_account_instr = system_instruction::create_account(
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        rent_for_extensions,
        mint_len as u64,
        &spl_token_2022::id(),
    );

    // Initialize metadata pointer extension
    let init_metadata_pointer_instr =
        spl_token_2022::extension::metadata_pointer::instruction::initialize(
            &spl_token_2022::id(),
            &mint_account.pubkey(),
            Some(mint_authority.pubkey()),
            Some(mint_account.pubkey()),
        )
        .unwrap();

    // Initialize the Mint Account data
    let init_mint_instr = spl_token_2022::instruction::initialize_mint(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        Some(&mint_authority.pubkey()),
        0,
    )
    .unwrap();

    let metadata_pointer_instr = spl_token_metadata_interface::instruction::initialize(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        name.into(),
        symbol.into(),
        uri.into(),
    );

    let update_metadata_pointer_instr = spl_token_metadata_interface::instruction::update_field(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        spl_token_metadata_interface::state::Field::Key("membership".into()),
        "FULL MEMBERSHIP RIGHTS".into(),
    );

    check_request_airdrop(&client, &mint_authority.pubkey(), 2);

    let message = Message::new(
        &[
            create_account_instr,
            init_metadata_pointer_instr,
            init_mint_instr,
            metadata_pointer_instr,
            update_metadata_pointer_instr,
        ],
        Some(&mint_authority.pubkey()),
    );
    let mut tx = Transaction::new_unsigned(message);
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    tx.sign(&[&mint_authority, &mint_account], recent_blockhash);
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
