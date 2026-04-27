use std::fs;
use std::path::Path;

use ark_bn254::Bn254;
use ark_groth16::{ProvingKey, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::CircuitSpecificSetupSNARK;
use rand::CryptoRng;

use crate::groth16::circuit::MSMCircuit;


pub fn generate_keys(n: usize, rng: &mut (impl rand::RngCore + CryptoRng)) -> Result<(ProvingKey<Bn254>, VerifyingKey<Bn254>), String> {
    let circuit = MSMCircuit::empty(n);
    ark_groth16::Groth16::<Bn254>::setup(circuit, rng)
        .map_err(|e| format!("Setup failed: {:?}", e))
}


pub fn save_proving_key(pk: &ProvingKey<Bn254>, path: impl AsRef<Path>) -> Result<(), String> {
    let mut bytes = Vec::new();
    pk.serialize_compressed(&mut bytes)
        .map_err(|e| format!("Failed to serialize proving key: {}", e))?;
    fs::write(path, bytes).map_err(|e| format!("Failed to write proving key: {}", e))
}

pub fn save_verifying_key(vk: &VerifyingKey<Bn254>, path: impl AsRef<Path>) -> Result<(), String> {
    let mut bytes = Vec::new();
    vk.serialize_compressed(&mut bytes)
        .map_err(|e| format!("Failed to serialize verifying key: {}", e))?;
    fs::write(path, bytes).map_err(|e| format!("Failed to write verifying key: {}", e))
}

pub fn load_proving_key(path: impl AsRef<Path>) -> Result<ProvingKey<Bn254>, String> {
    let bytes = fs::read(&path)
        .map_err(|e| format!("Failed to read proving key: {}", e))?;
    ProvingKey::<Bn254>::deserialize_compressed(&bytes[..])
        .map_err(|e| format!("Failed to deserialize proving key: {}", e))
}

pub fn load_verifying_key(path: impl AsRef<Path>) -> Result<VerifyingKey<Bn254>, String> {
    let bytes = fs::read(&path)
        .map_err(|e| format!("Failed to read verifying key: {}", e))?;
    VerifyingKey::<Bn254>::deserialize_compressed(&bytes[..])
        .map_err(|e| format!("Failed to deserialize verifying key: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_keys() {
        let mut rng = rand::rngs::OsRng;
        let n = 4;
        let result = generate_keys(n, &mut rng);
        assert!(result.is_ok(), "Key generation failed");
    }

    #[test]
    fn test_save_and_load_keys() {
        let mut rng = rand::rngs::OsRng;
        let n = 4;
        let (pk, vk) = generate_keys(n, &mut rng).expect("Key generation failed");

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let pk_path = temp_dir.path().join("proving_key.bin");
        let vk_path = temp_dir.path().join("verifying_key.bin");

        save_proving_key(&pk, &pk_path).expect("Failed to save proving key");
        save_verifying_key(&vk, &vk_path).expect("Failed to save verifying key");

        assert!(pk_path.exists(), "Proving key file not created");
        assert!(vk_path.exists(), "Verifying key file not created");

        let loaded_pk = load_proving_key(&pk_path).expect("Failed to load proving key");
        let loaded_vk = load_verifying_key(&vk_path).expect("Failed to load verifying key");


        let mut pk_bytes = Vec::new();
        pk.serialize_compressed(&mut pk_bytes).unwrap();
        let mut loaded_pk_bytes = Vec::new();
        loaded_pk.serialize_compressed(&mut loaded_pk_bytes).unwrap();
        assert_eq!(pk_bytes, loaded_pk_bytes, "Proving keys don't match after round-trip");

        let mut vk_bytes = Vec::new();
        vk.serialize_compressed(&mut vk_bytes).unwrap();
        let mut loaded_vk_bytes = Vec::new();
        loaded_vk.serialize_compressed(&mut loaded_vk_bytes).unwrap();
        assert_eq!(vk_bytes, loaded_vk_bytes, "Verifying keys don't match after round-trip");
    }
}
