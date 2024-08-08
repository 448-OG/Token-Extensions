use std::str::FromStr;

use solana_client::{rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::{
    instruction::AccountMeta, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, signature::Keypair,
    signer::Signer, system_instruction, transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address_with_program_id, instruction::create_associated_token_account,
};
use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList};
use spl_token_2022::{
    extension::{
        transfer_hook::{self},
        ExtensionType,
    },
    instruction::{initialize_mint, mint_to},
    offchain::create_transfer_checked_instruction_with_extra_metas,
    state::Mint,
};
use spl_transfer_hook_interface::{
    get_extra_account_metas_address,
    instruction::{execute, initialize_extra_account_meta_list, ExecuteInstruction},
};

#[tokio::main]
async fn main() {
    let transfer_hook_program_id =
        Pubkey::from_str("Arafvy1MtnvKJXif3dSE3PT2ZsFwW9qLmncJBh9d4G88").unwrap();
    // let mint_authority = Keypair::new();
    // let mint_account = Keypair::new();
    // let destination = Keypair::new();

    let mint_authority = Keypair::from_bytes(&MINT_AUTHORITY_BYTES).unwrap();
    let mint_account = Keypair::from_bytes(&MINT_ACCOUNT_BYTES).unwrap();
    let destination = Keypair::from_bytes(&DESTINATION_BYTES).unwrap();

    // println!(
    //     "MINT_AUTHORITY_BYTES: [u8;64] = {:?}",
    //     mint_authority.to_bytes()
    // );
    // println!(
    //     "MINT_ACCOUNT_BYTES: [u8;64] = {:?}",
    //     mint_account.to_bytes()
    // );
    // println!("DESTINATION_BYTES: [u8;64] = {:?}", destination.to_bytes());

    let decimals = 0u8;

    println!("TRANSFER HOOK PROGRAM: {}", &transfer_hook_program_id);
    println!("MINT AUTHORITY: {}", mint_authority.pubkey());
    println!("MINT ACCOUNT: {}", mint_account.pubkey());
    println!("Destination Keypair: {}", &destination.pubkey());
    println!("MINT Decimals : {}", decimals);

    let client = RpcClient::new("http://localhost:8899".to_string());

    let mint_extensions = [ExtensionType::TransferHook];
    let mint_size = ExtensionType::try_calculate_account_len::<Mint>(&mint_extensions).unwrap();
    let mint_rent = client
        .get_minimum_balance_for_rent_exemption(mint_size)
        .unwrap();

    let create_mint_account_instr = system_instruction::create_account(
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        mint_rent,
        mint_size as u64,
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

    let init_transfer_hook_instr = transfer_hook::instruction::initialize(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        Some(mint_authority.pubkey()),
        Some(transfer_hook_program_id),
    )
    .unwrap();

    let extra_account_metas_address =
        get_extra_account_metas_address(&mint_account.pubkey(), &transfer_hook_program_id);
    println!(
        "Extra Account Metas Address: {}",
        extra_account_metas_address
    );

    dbg!(&extra_account_metas_address);

    let mut all_instructions = vec![
        create_mint_account_instr,
        init_transfer_hook_instr,
        init_mint_instr,
    ];

    let extra_account_metas: [ExtraAccountMeta; 1] =
        [AccountMeta::new(transfer_hook_program_id, false).into()];

    let account_size = ExtraAccountMetaList::size_of(extra_account_metas.len()).unwrap();
    let required_lamports = client
        .get_minimum_balance_for_rent_exemption(account_size)
        .unwrap();
    let account_info = client.get_account(&extra_account_metas_address);
    let current_lamports = account_info.map(|a| a.lamports).unwrap_or(0);
    let transfer_lamports = required_lamports.saturating_sub(current_lamports);

    // Check if the extra meta account has already been initialized
    let extra_account_metas_account = client.get_account(&extra_account_metas_address);
    if let Ok(account) = &extra_account_metas_account {
        if account.owner != solana_program::system_program::id() {
            panic!("error: extra account metas for mint {} and program {transfer_hook_program_id} already exists", mint_account.pubkey());
        }
    }

    if transfer_lamports > 0 {
        all_instructions.push(system_instruction::transfer(
            &mint_authority.pubkey(),
            &extra_account_metas_address,
            transfer_lamports,
        ));
    }

    let init_extra_account_meta_instr = initialize_extra_account_meta_list(
        &transfer_hook_program_id,
        &extra_account_metas_address,
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        &extra_account_metas,
    );

    all_instructions.push(init_extra_account_meta_instr);

    check_request_airdrop(&client, &mint_authority.pubkey(), 2);

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &all_instructions,
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );

    dbg!(&client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap());

    let mint_authority_ata = get_associated_token_address_with_program_id(
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );
    println!("MINT AUTHORITY ATA: {}", mint_authority_ata);

    let destination_ata = get_associated_token_address_with_program_id(
        &destination.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    println!("Destination ATA: {}", &destination_ata);

    let mut rpc_config = RpcSendTransactionConfig::default();
    rpc_config.skip_preflight = true;

    {
        let mint_authority_ata_instr = create_associated_token_account(
            &mint_authority.pubkey(),
            &mint_authority.pubkey(),
            &mint_account.pubkey(),
            &spl_token_2022::id(),
        );
        let mint_to_source_instr = mint_to(
            &spl_token_2022::id(),
            &mint_account.pubkey(),
            &mint_authority_ata,
            &mint_authority.pubkey(),
            &[&mint_authority.pubkey()],
            2000,
        )
        .unwrap();

        let recent_blockhash = client.get_latest_blockhash().unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[mint_authority_ata_instr, mint_to_source_instr],
            Some(&mint_authority.pubkey()),
            &[&mint_authority],
            recent_blockhash,
        );

        dbg!(&client
            .send_and_confirm_transaction_with_spinner(&tx,)
            .unwrap());
    }

    {
        let destination_ata_instr = create_associated_token_account(
            &destination.pubkey(),
            &destination.pubkey(),
            &mint_account.pubkey(),
            &spl_token_2022::id(),
        );

        check_request_airdrop(&client, &destination.pubkey(), 1);

        let recent_blockhash = client.get_latest_blockhash().unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[destination_ata_instr],
            Some(&destination.pubkey()),
            &[&destination],
            recent_blockhash,
        );

        dbg!(&client
            .send_and_confirm_transaction_with_spinner(&tx)
            .unwrap());

        let mint_to_dest_ata_instr = mint_to(
            &spl_token_2022::id(),
            &mint_account.pubkey(),
            &destination_ata,
            &mint_authority.pubkey(),
            &[&mint_authority.pubkey()],
            1,
        )
        .unwrap();

        let recent_blockhash = client.get_latest_blockhash().unwrap();
        let tx = Transaction::new_signed_with_payer(
            &[mint_to_dest_ata_instr],
            Some(&mint_authority.pubkey()),
            &[&mint_authority],
            recent_blockhash,
        );

        dbg!(&client
            .send_and_confirm_transaction_with_spinner(&tx)
            .unwrap());
    }

    // Load the validation state data
    let validate_state_pubkey =
        get_extra_account_metas_address(&mint_account.pubkey(), &transfer_hook_program_id);
    dbg!(&validate_state_pubkey);
    let fetch_account_data_fn = |pubkey: Pubkey| async move {
        let inner_client = RpcClient::new("http://localhost:8899".to_string());
        Ok(Some(inner_client.get_account(&pubkey).unwrap().data))
    };
    let validate_state_data = fetch_account_data_fn(validate_state_pubkey)
        .await
        .unwrap()
        .unwrap();

    let mint_authority_ata = get_associated_token_address_with_program_id(
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );
    println!("MINT AUTHORITY ATA: {}", mint_authority_ata);

    let destination_ata = get_associated_token_address_with_program_id(
        &destination.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    println!("Destination ATA: {}", &destination_ata);

    let amount_to_transfer = 31u64;

    // First create an `ExecuteInstruction`
    let mut execute_instruction = execute(
        &spl_token_2022::id(),
        &mint_authority_ata,
        &mint_account.pubkey(),
        &destination_ata,
        &mint_authority.pubkey(),
        &validate_state_pubkey,
        amount_to_transfer,
    );

    // Resolve all additional required accounts for `ExecuteInstruction`
    ExtraAccountMetaList::add_to_instruction::<ExecuteInstruction, _, _>(
        &mut execute_instruction,
        fetch_account_data_fn,
        &validate_state_data,
    )
    .await
    .unwrap();

    let transfer_instr = create_transfer_checked_instruction_with_extra_metas(
        &spl_token_2022::id(),
        &mint_authority_ata,
        &mint_account.pubkey(),
        &destination_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey()],
        4,
        decimals,
        fetch_account_data_fn,
    )
    .await
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[transfer_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority],
        recent_blockhash,
    );

    dbg!(&client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap());
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

const MINT_AUTHORITY_BYTES: [u8; 64] = [
    145, 10, 83, 58, 145, 215, 127, 168, 166, 74, 48, 245, 188, 223, 90, 152, 114, 104, 107, 142,
    63, 113, 73, 237, 135, 31, 172, 138, 245, 155, 154, 66, 197, 233, 42, 101, 111, 36, 55, 36,
    132, 32, 20, 54, 203, 4, 71, 164, 148, 40, 215, 246, 52, 181, 14, 155, 188, 139, 243, 179, 20,
    93, 66, 203,
];
const MINT_ACCOUNT_BYTES: [u8; 64] = [
    185, 71, 107, 189, 192, 172, 42, 180, 162, 72, 248, 162, 203, 214, 218, 206, 170, 226, 140, 68,
    166, 209, 186, 127, 200, 150, 202, 15, 76, 72, 93, 109, 117, 140, 183, 120, 147, 140, 99, 161,
    70, 108, 101, 115, 250, 21, 190, 159, 78, 38, 71, 148, 86, 32, 87, 129, 220, 64, 94, 64, 45,
    101, 206, 144,
];
const DESTINATION_BYTES: [u8; 64] = [
    236, 239, 238, 157, 136, 10, 77, 194, 181, 1, 120, 244, 42, 222, 158, 106, 40, 248, 29, 127,
    95, 128, 243, 94, 230, 235, 55, 93, 222, 5, 111, 193, 58, 177, 81, 73, 159, 222, 213, 113, 54,
    153, 17, 98, 238, 182, 125, 112, 248, 156, 23, 74, 15, 84, 20, 164, 210, 110, 41, 251, 247,
    110, 14, 162,
];
