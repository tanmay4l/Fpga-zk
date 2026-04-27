use anyhow::{anyhow, Context, Result};
use fpga_zk::groth16;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let batch_size = if args.len() > 1 {
        args[1]
            .parse::<usize>()
            .context("batch size must be a positive integer")?
    } else {
        4
    };

    let output_dir = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        PathBuf::from(".")
    };

    eprintln!("Generating Groth16 keys for batch size {}", batch_size);
    eprintln!("Output directory: {}", output_dir.display());

    let mut rng = rand::rngs::OsRng;
    let (pk, vk) = groth16::generate_keys(batch_size, &mut rng)
        .map_err(|e| anyhow!("{}", e))?;

    let pk_path = output_dir.join("proving_key.bin");
    let vk_path = output_dir.join("verifying_key.bin");

    groth16::save_proving_key(&pk, &pk_path)
        .map_err(|e| anyhow!("{}", e))?;
    groth16::save_verifying_key(&vk, &vk_path)
        .map_err(|e| anyhow!("{}", e))?;

    eprintln!("✓ Proving key saved to {}", pk_path.display());
    eprintln!("✓ Verifying key saved to {}", vk_path.display());
    eprintln!();
    eprintln!("Run daemon with:");
    eprintln!("  FPGA_ZK_PK={} cargo run --bin fpga-zk-daemon", pk_path.display());

    Ok(())
}
