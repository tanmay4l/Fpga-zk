# FPGA-ZK: Zero-Knowledge Proof Accelerator for Solana

Hardware-accelerated Groth16 zero-knowledge proofs on Solana blockchain. Proves correct computation off-chain, verifies cheaply on-chain using BN254 native precompiles.

## Architecture

```
┌─────────────────────────────────────┐
│  Client {points, scalars (BN254)}   │
└──────────────┬──────────────────────┘
               │ TCP (framed JSON)
               ▼
┌──────────────────────────────────────────────────┐
│  Daemon                                          │
│  ├─ 1. Compute MSM (BN254 Pippenger)             │
│  ├─ 2. Generate Groth16 proof (ark-groth16)      │
│  │       Circuit: ∑ scalar_i · point_i = result  │
│  │       FPGA accelerates MSM inside prover      │
│  └─ 3. Return {result, proof_a, proof_b, ...}    │
└──────────────┬──────────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────────────┐
│  Solana Program                                  │
│  ├─ alt_bn128_pairing (native precompile)        │
│  ├─ Verify: e(A,B) == e(α,β)·e(Σpublic,γ)·e(C,δ)│
│  └─ Emit ProofVerified { batch_size }            │
└──────────────────────────────────────────────────┘
```

## Quick Start

### Build
```bash
cargo build
cargo build-sbf  # Solana program
```

### Run Daemon
```bash
FPGA_ZK_ADDR=127.0.0.1:9000 cargo run --bin fpga-zk-daemon
```

### Test
```bash
cargo test                                            # Unit tests
cargo test --test daemon_integration -- --ignored     # TCP integration
cargo test --test validator_test -- --ignored         # Solana devnet
cargo test --test groth16_integration -- --nocapture  # Groth16 prove/verify
```

## ZK Proof System (Groth16)

### Prover (Daemon)
- Takes: points (BN254 G1), scalars (BN254 Fr)
- Computes: MSM using Pippenger (4.6x speedup with FPGA)
- Proves: knowledge of scalars via Groth16 circuit
- Returns: (result, proof_a, proof_b, proof_c, public_inputs)

### Verifier (Solana)
- Uses: `alt_bn128` native precompile for pairing checks
- Cost: ~300K CU (well under 1.4M limit)
- Verification equation: `e(A,B) == e(α,β) · e(Σpublic,γ) · e(C,δ)`

## Protocol

### TCP Framing
```
[4-byte LE length][JSON payload]
```

### Prove Request
```json
{
  "op": "prove",
  "points": [["bn254_g1_bytes"], ...],
  "scalars": [["bn254_fr_bytes"], ...]
}
```

### Prove Response
```json
{
  "result": "bn254_g1_bytes",
  "proof_a": "64_bytes",
  "proof_b": "128_bytes",
  "proof_c": "64_bytes",
  "public_inputs": [["point_bytes"], ["result_bytes"]],
  "error": null
}
```

### MSM Request (legacy)
```json
{
  "op": "msm",
  "points": [["bls12381_g1_bytes"], ...],
  "scalars": [["bls12381_fr_bytes"], ...]
}
```

## Constraints

- **Prove operation**: 8 points max (per Solana CU limits)
- **BN254**: G1 (32 bytes compressed), Fr (32 bytes)
- **BLS12-381**: G1 (48 bytes compressed), Fr (32 bytes)
- **Max message**: 4 MB
- **Proof verification CU**: ~300K (3x cheaper than naive approach)

## Hardware

KV260 FPGA acceleration (optional, via `--features fpga`):
- Accelerates G1 MSM inside the Groth16 prover
- Falls back to software Pippenger on x86 dev machines
- No hardware required for Solana verification (uses native precompile)

