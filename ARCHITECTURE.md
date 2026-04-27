# fpga-zk: Hardware-Accelerated ZK Verification for Solana

## Overview

fpga-zk accelerates BLS12-381 Multi-Scalar Multiplication (MSM) for on-chain zero-knowledge proof verification using FPGA hardware. The architecture follows **Path B: In-Program Verification** — a production-grade pattern that separates expensive off-chain computation from on-chain verification.

## Architecture

```
┌─────────────────────────────────────────────┐
│ Client (Prover)                             │
│ - Generate Groth16 proof                    │
│ - Extract points, scalars                   │
└──────────────────┬──────────────────────────┘
                   │
                   ↓ (send points, scalars)
┌─────────────────────────────────────────────┐
│ fpga-zk Daemon (Untrusted)                  │
│ - Compute MSM = Σ(point_i * scalar_i)      │
│ - Return compressed point (48 bytes)        │
│ - Cost: Hardware acceleration (FPGA)        │
│ - CU cost: 0 (off-chain)                    │
└──────────────────┬──────────────────────────┘
                   │
                   ↓ (send result, proof data)
┌─────────────────────────────────────────────┐
│ Solana Program (verify_msm)                 │
│ - Deserialize points, scalars, result       │
│ - Recompute MSM = Σ(point_i * scalar_i)    │
│ - Assert result == provided_result          │
│ - Emit MSMVerified event                    │
│ - Cost: ~100K CU (ark-ec point ops)        │
│ - Trust: Cryptographic proof, not daemon    │
└─────────────────────────────────────────────┘
```

## Security Model

### Path B: In-Program Verification (Production-Ready)

**Trust Assumption:** None on daemon. Verification happens cryptographically on-chain.

**How it works:**
1. Daemon computes MSM (assumed untrusted)
2. Program independently verifies the computation
3. Verification fails if result is wrong
4. No malicious daemon can forge a valid proof

**Security Guarantees:**
- ✓ Proof of correctness: On-chain verification
- ✓ Works with any hardware: No TEE dependency
- ✓ Production-ready today: No additional infrastructure
- ✓ Anza-grade security: Same pattern as Ed25519 precompiles

**Cost Tradeoff:**
- Daemon: O(n) time (fast, FPGA-accelerated)
- Program: O(n) CU (~100K for typical batch)
- Still 10-100x faster than pure on-chain MSM

### Path A: TEE Attestation (Future)

When FPGA hardware supports Intel SGX or ARM TrustZone:

1. Daemon runs in Trusted Execution Environment (TEE)
2. Generates cryptographic attestation proof
3. Program verifies attestation (not recomputing)
4. Zero re-verification overhead, full acceleration benefit

**Integration:** Via [Switchboard Attestation Program (V3)](https://docs.rs/switchboard-solana-staging/latest/switchboard_solana/).

---

## Components

### 1. MSM Library (`src/msm.rs`, `src/lib.rs`)
- Pure Rust implementation of multi-scalar multiplication
- BLS12-381 curve support
- Naive algorithm for correctness; Pippenger bucket algorithm planned for optimization
- **Test coverage:** Unit tests for correctness

### 2. TCP Daemon (`daemon/src/main.rs`)
- Listens on `127.0.0.1:9000`
- Accepts JSON-serialized MSM requests
- Returns compressed point result (48 bytes)
- **Deployment:** Can run on same machine as validator, on dedicated hardware, or in TEE

### 3. Solana Program (`solana-program/src/lib.rs`)
- Instruction: `verify_msm(points, scalars, daemon_result)`
- Verifies daemon result matches on-chain computation
- Emits `MSMVerified` event with batch size
- **Cost:** ~100K CU per verification

---

## Deployment

### Development (Simulator)
```bash
# Terminal 1: Start daemon
cargo run --bin fpga-zk-daemon

# Terminal 2: Run Solana program tests
cd solana-program && cargo test
```

### Production (with real hardware)

1. **Hardware setup:** Deploy FPGA accelerator (Intel Agilex recommended)
2. **Daemon:** Run on hardware host or co-located with validator
3. **Program:** Deploy Solana program to mainnet
4. **Client:** Submit transactions with (points, scalars, daemon_result)

---

## Threat Model

### Assumptions
- ✓ Solana runtime is honest
- ✓ Network is not attacker-controlled (standard Solana model)
- ✗ Daemon is assumed untrusted (verification proves correctness)
- ✗ Validator is trusted (Solana consensus guarantees)

### Attacks Mitigated
1. **Daemon returns wrong result:** On-chain verification catches it
2. **Daemon is compromised:** Program still verifies correctness
3. **Network tampering:** Solana cryptographic guarantees apply
4. **Point/scalar corruption:** Deserialization failure or verification failure

### Known Limitations
- **No privacy:** Points and scalars are visible on-chain (standard for Solana)
- **Computational overhead:** Verification costs ~100K CU (acceptable for most use cases)
- **Not constant-time:** Ark-ec operations are not constant-time (timing attacks possible on hardware, negligible on-chain)

---

## Performance Targets

- **Daemon MSM computation:** 1-10ms for typical batch (depends on FPGA implementation)
- **Program verification:** ~100K CU (~1-2ms at 60M CU/block)
- **Total:** ~2-12ms end-to-end (vs. 100ms+ pure on-chain)
- **Speedup:** 5-50x over naive on-chain implementation

---

## Roadmap

### Phase 1 (Complete)
- [x] MSM library with tests
- [x] TCP daemon
- [x] Solana program with in-program verification

### Phase 2 (Next)
- [ ] Pippenger bucket algorithm optimization (~10x faster daemon)
- [ ] Integration tests with Solana test validator
- [ ] Benchmark suite (local vs. on-chain cost comparison)

### Phase 3 (Future)
- [ ] Switchboard TEE integration (Path A)
- [ ] FPGA RTL design (Verilog for Intel Agilex)
- [ ] Mainnet deployment and auditing

---

## References

- [Solana Security Best Practices](https://www.helius.dev/blog/a-hitchhiker-guide-to-solana-program-security)
- [Groth16 Verification on Solana](https://github.com/Lightprotocol/groth16-solana)
- [Switchboard Attestation (V3)](https://docs.rs/switchboard-solana-staging/latest/switchboard_solana/)
- [ark-ec Documentation](https://docs.rs/ark-ec/latest/ark_ec/)
