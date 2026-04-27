use ark_bls12_381::{Fr, G1Projective};
use ark_ec::CurveGroup;
use ark_ff::UniformRand;
use ark_serialize::CanonicalSerialize;
use rand::rngs::OsRng;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use std::str::FromStr;
use borsh::BorshSerialize;

#[test]
#[ignore]
fn test_validator_msm_program() {
    let rpc_url = "https://api.devnet.solana.com";
    println!("\n=== Real Validator Test: MSM Program (Devnet) ===");
    println!("RPC URL: {}", rpc_url);

    let client = RpcClient::new(rpc_url.to_string());

    let mut ready = false;
    for attempt in 0..30 {
        match client.get_version() {
            Ok(v) => {
                println!("✓ Connected to validator ({})", v.solana_core);
                ready = true;
                break;
            }
            Err(_) if attempt < 29 => {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            Err(e) => {
                eprintln!("✗ Cannot connect after 30 seconds: {}", e);
                panic!("Validator not ready");
            }
        }
    }
    if !ready {
        panic!("Failed to connect to validator");
    }

    let keypair_path = std::path::Path::new(&std::env::var("HOME").unwrap())
        .join(".config/solana/id.json");

    let payer = read_keypair_file(&keypair_path)
        .expect("Failed to read keypair");

    println!("✓ Payer: {}", payer.pubkey());

    let recent_blockhash = client
        .get_latest_blockhash()
        .expect("Failed to get blockhash");

    let program_id = Pubkey::from_str("Atxwqo8Xtn2U6mtRkcyzwUFToVjcfabQsR81fRBbuQCu")
        .expect("Invalid program ID");

    match client.get_account(&program_id) {
        Ok(account) => {
            println!("✓ Program deployed (size: {} bytes, owner: {})",
                account.data.len(),
                account.owner);
        }
        Err(_) => {
            eprintln!("✗ Program not found - not deployed to validator");
            println!("\nTo fix: Start validator with:");
            println!("  solana-test-validator --rpc-port 8899 \\");
            println!("    --bpf-program Atxwqo8Xtn2U6mtRkcyzwUFToVjcfabQsR81fRBbuQCu \\");
            println!("    solana-program/target/deploy/fpga_zk_solana.so");
            panic!("Program not deployed");
        }
    }

    let batch_size = 4;
    println!("\n--- Batch size: {} points ---", batch_size);

    let mut rng = OsRng;
    let points: Vec<_> = (0..batch_size)
        .map(|_| G1Projective::rand(&mut rng).into_affine())
        .collect();
    let scalars: Vec<_> = (0..batch_size).map(|_| Fr::rand(&mut rng)).collect();

    let computed: G1Projective = points
        .iter()
        .zip(&scalars)
        .map(|(p, s)| G1Projective::from(*p) * s)
        .sum();

    let mut points_bytes = Vec::new();
    let mut scalars_bytes = Vec::new();
    let mut result_bytes = Vec::new();

    for point in &points {
        let mut buf = Vec::new();
        point.serialize_compressed(&mut buf).unwrap();
        points_bytes.push(buf);
    }

    for scalar in &scalars {
        let mut buf = Vec::new();
        scalar.serialize_compressed(&mut buf).unwrap();
        scalars_bytes.push(buf);
    }

    computed.into_affine()
        .serialize_compressed(&mut result_bytes)
        .unwrap();

    println!("✓ Generated test data: {} points, result {} bytes", batch_size, result_bytes.len());

    #[derive(BorshSerialize)]
    struct VerifyMsmArgs {
        points_bytes: Vec<Vec<u8>>,
        scalars_bytes: Vec<Vec<u8>>,
        result_bytes: Vec<u8>,
    }

    let args = VerifyMsmArgs {
        points_bytes,
        scalars_bytes,
        result_bytes,
    };

    let mut data = vec![0x4e, 0x4e, 0x76, 0xf7, 0xe1, 0x96, 0x5f, 0xdd];
    args.serialize(&mut data).expect("Serialize failed");

    let compute_budget_instruction = solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);

    let msm_instruction = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data,
    };

    let mut tx = Transaction::new_unsigned(Message::new(
        &[compute_budget_instruction, msm_instruction],
        Some(&payer.pubkey())
    ));
    tx.sign(&[&payer], recent_blockhash);

    println!("\n--- Sending transaction ---");
    match client.send_and_confirm_transaction(&tx) {
        Ok(sig) => {
            println!("✓ Transaction confirmed: {}", sig);

            use solana_transaction_status::UiTransactionEncoding;
            match client.get_transaction(&sig, UiTransactionEncoding::Binary) {
                Ok(tx_result) => {
                    if let Some(meta) = tx_result.transaction.meta {
                        match meta.compute_units_consumed {
                            solana_transaction_status::option_serializer::OptionSerializer::Some(cu) => {
                                println!("\n✓✓✓ PROGRAM VERIFIED ON-CHAIN ✓✓✓");
                                println!("CU Consumed: {}", cu);
                                println!("CU Per Point: {}", cu / batch_size as u64);
                                println!("Status: Success");
                            }
                            _ => println!("✓ Transaction succeeded (CU data unavailable)"),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning retrieving tx: {}", e);
                    println!("✓ Transaction likely succeeded (could not fetch metadata)");
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Transaction failed: {}", e);
            eprintln!("\nDebug info:");
            eprintln!("  Program ID: {}", program_id);
            eprintln!("  Payer: {}", payer.pubkey());
            eprintln!("  System Program: {}", solana_sdk::system_program::ID);

            eprintln!("\nAttempting to fetch detailed logs...");
            if let Ok(sim_result) = client.simulate_transaction(&tx) {
                if let Some(logs) = &sim_result.value.logs {
                    eprintln!("Program logs:");
                    for log in logs {
                        eprintln!("  {}", log);
                    }
                }
            }
            panic!("Transaction failed");
        }
    }
}
