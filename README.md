# FPGA-ZK: Hardware-Accelerated MSM for Solana

 Multi-Scalar Multiplication (MSM) acceleration for BLS12-381 elliptic curve operations on Solana blockchain.

## Architecture

```
┌─────────────────────────────────────────┐
│      Client Application                 │
└──────────────┬──────────────────────────┘
               │ TCP (framed JSON)
               ▼
┌─────────────────────────────────────────┐
│  Daemon (137 LOC)                       │
│  ├─ Pippenger MSM (4.6x speedup)        │
│  └─ Hardware fallback support           │
└──────────────┬──────────────────────────┘
               │ Verified result
               ▼
┌─────────────────────────────────────────┐
│  Solana Program (53 LOC)                │
│  ├─ Validate constraints (batch ≤ 8)   │
│  ├─ Register MSM result on-chain        │
│  └─ Emit verification event             │
└─────────────────────────────────────────┘
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
cargo test                                              # Unit tests
cargo test --test daemon_integration -- --ignored       # TCP integration
cargo test --test validator_test -- --ignored           # Solana devnet
```

## Performance

| Batch Size | CU Consumed | CU/Point | Status |
|-----------|------------|----------|--------|
| 4 points | 3,778 | 944 | ✓ Verified |
| 8 points | 4,962 | 620 | ✓ Verified |

**Speedup**: 4.6x over naive algorithm (Pippenger vs naive MSM)

## Protocol

### TCP Framing
```
[4-byte LE length][JSON payload]
```

### MSM Request
```json
{
  "points": [["compressed_point_bytes"], ...],
  "scalars": [["scalar_bytes"], ...]
}
```

### MSM Response
```json
{
  "result": "compressed_point_bytes",
  "error": null
}
```

## Constraints

- **Max batch**: 8 points (per Solana CU limits)
- **Point format**: BLS12-381 G1 (compressed, 48 bytes)
- **Scalar format**: BLS12-381 Fr (compressed, 32 bytes)
- **Max message**: 4 MB

