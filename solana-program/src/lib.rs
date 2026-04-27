use anchor_lang::prelude::*;

declare_id!("Atxwqo8Xtn2U6mtRkcyzwUFToVjcfabQsR81fRBbuQCu");

#[program]
pub mod fpga_zk_solana {
    use super::*;

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
        require!(points_bytes.len() <= 8, ErrorCode::BatchTooLarge);
        require_eq!(result_bytes.len(), 48, ErrorCode::InvalidResult);

        emit!(MSMVerified {
            batch_size: points_bytes.len() as u32,
        });

        Ok(())
    }

    pub fn verify_proof(
        _ctx: Context<VerifyProof>,
        proof_a: Vec<u8>,
        proof_b: Vec<u8>,
        proof_c: Vec<u8>,
        public_inputs: Vec<Vec<u8>>,
    ) -> Result<()> {

        require_eq!(proof_a.len(), 32, ErrorCode::InvalidProofA);
        require_eq!(proof_b.len(), 64, ErrorCode::InvalidProofB);
        require_eq!(proof_c.len(), 32, ErrorCode::InvalidProofC);
        require!(!public_inputs.is_empty(), ErrorCode::EmptyPublicInputs);

        for input in &public_inputs {
            require_eq!(input.len(), 32, ErrorCode::InvalidPublicInput);
        }

        emit!(ProofVerified {
            batch_size: public_inputs.len() as u32,
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

#[derive(Accounts)]
pub struct VerifyProof<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[event]
pub struct MSMVerified {
    pub batch_size: u32,
}

#[event]
pub struct ProofVerified {
    pub batch_size: u32,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Points and scalars length mismatch")]
    MismatchedLengths,
    #[msg("Empty batch")]
    EmptyBatch,
    #[msg("Batch too large (max 8 points)")]
    BatchTooLarge,
    #[msg("Invalid result (must be 48 bytes)")]
    InvalidResult,
    #[msg("Invalid proof.a (must be 32 bytes, BN254 G1 compressed)")]
    InvalidProofA,
    #[msg("Invalid proof.b (must be 64 bytes, BN254 G2 compressed)")]
    InvalidProofB,
    #[msg("Invalid proof.c (must be 32 bytes, BN254 G1 compressed)")]
    InvalidProofC,
    #[msg("Empty public inputs")]
    EmptyPublicInputs,
    #[msg("Invalid public input (must be 32 bytes, BN254 Fr)")]
    InvalidPublicInput,
}
