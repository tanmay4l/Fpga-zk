use anchor_lang::prelude::*;
use ark_bls12_381::{Fr, G1Affine, G1Projective};
use ark_ec::{CurveGroup, Group};
use ark_ff::{PrimeField, Zero};
use ark_serialize::CanonicalDeserialize;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod fpga_zk_solana {
    use super::*;

    /// Verify that daemon-computed MSM result is correct.
    /// Computes MSM independently on-chain, compares with provided result.
    /// WARNING: This consumes ~100K CU for verification. The speedup comes from
    /// daemon doing the expensive MSM computation off-chain.
    pub fn verify_msm(
        _ctx: Context<VerifyMSM>,
        points_bytes: Vec<Vec<u8>>,
        scalars_bytes: Vec<Vec<u8>>,
        result_bytes: Vec<u8>,
    ) -> Result<()> {
        require!(
            points_bytes.len() == scalars_bytes.len(),
            ErrorCode::MismatchedLengths
        );
        require!(!points_bytes.is_empty(), ErrorCode::EmptyBatch);
        require!(points_bytes.len() <= 255, ErrorCode::BatchTooLarge);

        let points = deserialize_points(&points_bytes)?;
        let scalars = deserialize_scalars(&scalars_bytes)?;
        let result = deserialize_point(&result_bytes)?;

        let computed = compute_msm(&points, &scalars);

        require_eq!(
            computed.into_affine(),
            result,
            ErrorCode::VerificationFailed
        );

        emit!(MSMVerified {
            batch_size: points_bytes.len() as u32,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct VerifyMSM<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[event]
pub struct MSMVerified {
    pub batch_size: u32,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Points and scalars length mismatch")]
    MismatchedLengths,
    #[msg("Empty batch")]
    EmptyBatch,
    #[msg("Batch too large")]
    BatchTooLarge,
    #[msg("Invalid point deserialization")]
    InvalidPoint,
    #[msg("Invalid scalar deserialization")]
    InvalidScalar,
    #[msg("Invalid result deserialization")]
    InvalidResult,
    #[msg("MSM verification failed: daemon result is incorrect")]
    VerificationFailed,
}

fn deserialize_points(points_bytes: &[Vec<u8>]) -> Result<Vec<G1Affine>> {
    points_bytes
        .iter()
        .map(|b| {
            G1Affine::deserialize_compressed(b.as_slice())
                .map_err(|_| error!(ErrorCode::InvalidPoint))
        })
        .collect()
}

fn deserialize_scalars(scalars_bytes: &[Vec<u8>]) -> Result<Vec<Fr>> {
    scalars_bytes
        .iter()
        .map(|b| {
            Fr::deserialize_compressed(b.as_slice())
                .map_err(|_| error!(ErrorCode::InvalidScalar))
        })
        .collect()
}

fn deserialize_point(result_bytes: &[u8]) -> Result<G1Affine> {
    G1Affine::deserialize_compressed(result_bytes)
        .map_err(|_| error!(ErrorCode::InvalidResult))
}

fn compute_msm(points: &[G1Affine], scalars: &[Fr]) -> G1Projective {
    if points.is_empty() {
        return G1Projective::zero();
    }

    points
        .iter()
        .zip(scalars.iter())
        .map(|(p, s)| G1Projective::from(*p) * *s)
        .sum()
}
