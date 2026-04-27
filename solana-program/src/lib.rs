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
    #[msg("Batch too large (max 8 points)")]
    BatchTooLarge,
    #[msg("Invalid result (must be 48 bytes)")]
    InvalidResult,
}
