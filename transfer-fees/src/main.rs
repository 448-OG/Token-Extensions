use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey,
    signature::Keypair, signer::Signer, system_instruction, transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address_with_program_id, instruction::create_associated_token_account,
};
use spl_token_2022::{
    extension::{
        transfer_fee::{
            instruction::{
                harvest_withheld_tokens_to_mint, initialize_transfer_fee_config,
                transfer_checked_with_fee, withdraw_withheld_tokens_from_accounts,
                withdraw_withheld_tokens_from_mint,
            },
            TransferFeeAmount,
        },
        BaseStateWithExtensions, ExtensionType, StateWithExtensions,
    },
    instruction::mint_to,
    state::{Account, Mint},
};

fn main() {
    let mint_authority = Keypair::new();
    let mint_account = Keypair::new();

    println!("MINT ACCOUNT: {}", mint_account.pubkey());
    let decimals = 0u8;
    // Fee basis points for transfers (100 = 1%)
    let fee_basis_points = 100u16;
    // Maximum fee for transfers in token base units
    let max_fee = 100u64;

    let client = RpcClient::new("http://localhost:8899".to_string());

    let extensions = [ExtensionType::TransferFeeConfig];
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

    let transfer_fee_config_instr = initialize_transfer_fee_config(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        Some(&mint_authority.pubkey()),
        Some(&mint_authority.pubkey()),
        fee_basis_points,
        max_fee,
    )
    .unwrap();

    // Initialize the Mint Account data
    let init_mint_instr = spl_token_2022::instruction::initialize_mint(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority.pubkey(),
        Some(&mint_authority.pubkey()),
        decimals,
    )
    .unwrap();

    check_request_airdrop(&client, &mint_authority.pubkey(), 2);

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[
            create_account_instr,
            transfer_fee_config_instr,
            init_mint_instr,
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
    check_request_airdrop(&client, &destination.pubkey(), 1);

    let mint_authority_ata_instr = create_associated_token_account(
        &mint_authority.pubkey(),
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );
    let destination_ata_instr = create_associated_token_account(
        &destination.pubkey(),
        &destination.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    let mint_authority_ata = get_associated_token_address_with_program_id(
        &mint_authority.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );
    let destination_ata = get_associated_token_address_with_program_id(
        &destination.pubkey(),
        &mint_account.pubkey(),
        &spl_token_2022::id(),
    );

    let mint_to_instr = mint_to(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        200_000,
    )
    .unwrap();
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[
            mint_authority_ata_instr,
            destination_ata_instr,
            mint_to_instr,
        ],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account, &destination],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();
    println!("MINT AUTHORITY ATA: {}", &mint_authority_ata);
    println!("DESTINATION ATA: {}", &destination_ata);

    let (party_keypairs, party_atas) = many_atas(&client, &mint_account.pubkey());
    let last_party_keypair = party_keypairs.last().unwrap();
    let last_party_ata = party_atas.last().unwrap();
    println!("LAST PUBKEY: {}", &last_party_keypair.pubkey());
    println!("LAST ATA: {}", &last_party_ata);

    let transfer_amount = 100_000u64;
    let fee = ((transfer_amount as f64 * fee_basis_points as f64) / 10_000f64) as u64;
    let fee = if fee > max_fee { max_fee } else { fee };

    let transfer_instr = transfer_checked_with_fee(
        &spl_token_2022::id(),
        &mint_authority_ata,
        &mint_account.pubkey(),
        &destination_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        transfer_amount,
        decimals,
        fee,
    )
    .unwrap();
    let transfer_instr_last = transfer_checked_with_fee(
        &spl_token_2022::id(),
        &mint_authority_ata,
        &mint_account.pubkey(),
        &last_party_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        transfer_amount,
        decimals,
        fee,
    )
    .unwrap();
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[transfer_instr, transfer_instr_last],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    dbg!("TRANSFER_FEE_DONE");
    let program_accounts = client.get_program_accounts(&spl_token_2022::id()).unwrap();
    let token_accounts = program_accounts
        .iter()
        .filter_map(|(pubkey, account)| {
            if let Ok(token_account) = StateWithExtensions::<Account>::unpack(&account.data) {
                if token_account.base.mint == mint_account.pubkey() {
                    Some((pubkey, token_account))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<(&Pubkey, StateWithExtensions<Account>)>>();

    let withheld_fees_accounts = token_accounts
        .iter()
        .take_while(|(pubkey, _)| *pubkey != last_party_ata)
        .filter_map(|(pubkey, token_account)| {
            let transfer_fee_amount = token_account.get_extension::<TransferFeeAmount>().unwrap();
            let amount: u64 = transfer_fee_amount.withheld_amount.into();

            if amount > 0 {
                Some(*pubkey)
            } else {
                None
            }
        })
        .collect::<Vec<&Pubkey>>();

    let withdraw_withheld_instr = withdraw_withheld_tokens_from_accounts(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
        withheld_fees_accounts.as_slice(),
    )
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[withdraw_withheld_instr],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    dbg!("WITHDRAW_FEE_DONE");

    let harvest_instr = harvest_withheld_tokens_to_mint(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &[last_party_ata],
    )
    .unwrap();

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[harvest_instr],
        Some(&last_party_keypair.pubkey()),
        &[&last_party_keypair],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();
    dbg!("HARVEST_FEE_DONE");

    let withdraw_withheld = withdraw_withheld_tokens_from_mint(
        &spl_token_2022::id(),
        &mint_account.pubkey(),
        &mint_authority_ata,
        &mint_authority.pubkey(),
        &[&mint_authority.pubkey(), &mint_account.pubkey()],
    )
    .unwrap();
    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[withdraw_withheld],
        Some(&mint_authority.pubkey()),
        &[&mint_authority, &mint_account],
        recent_blockhash,
    );
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    dbg!("ALL_DONE");
}

fn many_atas(client: &RpcClient, mint_account_address: &Pubkey) -> (Vec<Keypair>, Vec<Pubkey>) {
    let party_keypairs = (0u8..2).map(|_| Keypair::new()).collect::<Vec<Keypair>>();
    let mut party_atas = Vec::<Pubkey>::new();

    for party in &party_keypairs {
        let party_ata = get_associated_token_address_with_program_id(
            &party.pubkey(),
            &mint_account_address,
            &spl_token_2022::id(),
        );
        println!("PARTY ATA: {}", party_ata);
        party_atas.push(party_ata);

        let party_ata_instr = create_associated_token_account(
            &party.pubkey(),
            &party.pubkey(),
            mint_account_address,
            &spl_token_2022::id(),
        );

        check_request_airdrop(client, &party.pubkey(), 1);

        let recent_blockhash = client.get_latest_blockhash().unwrap();

        let tx = Transaction::new_signed_with_payer(
            &[party_ata_instr],
            Some(&party.pubkey()),
            &[&party],
            recent_blockhash,
        );
        client
            .send_and_confirm_transaction_with_spinner(&tx)
            .unwrap();
    }

    (party_keypairs, party_atas)
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
