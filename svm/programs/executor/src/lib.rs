use anchor_lang::prelude::*;

declare_id!("Ax7mtQPbNPQmghd7C3BHrMdwwmkAXBDq7kNGfXNcc7dg");

// TODO: cfg_if
static OUR_CHAIN: u16 = 1;

static SIGNED_QUOTE_HEADER_LEN: usize = 68;

#[program]
pub mod executor {
    use super::*;

    pub fn request_for_execution(
        ctx: Context<RequestForExecution>,
        args: RequestForExecutionArgs,
    ) -> Result<()> {
        let RequestForExecutionArgs {
            amount,
            dst_chain,
            signed_quote_bytes,
            ..
        } = args;
        require!(
            signed_quote_bytes.len() >= SIGNED_QUOTE_HEADER_LEN,
            ExecutorErrors::InvalidArguments
        );
        {
            let quote_src_chain = u16::from_be_bytes(
                signed_quote_bytes[56..58]
                    .try_into()
                    .map_err(|_| ExecutorErrors::InvalidArguments)?,
            );
            let quote_dst_chain = u16::from_be_bytes(
                signed_quote_bytes[58..60]
                    .try_into()
                    .map_err(|_| ExecutorErrors::InvalidArguments)?,
            );
            let expiry_time = u64::from_be_bytes(
                signed_quote_bytes[60..68]
                    .try_into()
                    .map_err(|_| ExecutorErrors::InvalidArguments)?,
            );
            require!(
                quote_src_chain == OUR_CHAIN,
                ExecutorErrors::QuoteSrcChainMismatch
            );
            require!(
                quote_dst_chain == dst_chain,
                ExecutorErrors::QuoteDstChainMismatch,
            );
            require!(
                expiry_time
                    > Clock::get()?
                        .unix_timestamp
                        .try_into()
                        .map_err(|_| ExecutorErrors::QuoteExpired)?,
                ExecutorErrors::QuoteExpired
            );
        }
        require!(
            ctx.accounts.payee.key.as_ref() == &signed_quote_bytes[24..56],
            ExecutorErrors::QuotePayeeMismatch
        );

        let from_account = &ctx.accounts.payer;
        let to_account = &ctx.accounts.payee;

        let transfer_instruction = anchor_lang::solana_program::system_instruction::transfer(
            from_account.key,
            to_account.key,
            amount,
        );

        anchor_lang::solana_program::program::invoke_signed(
            &transfer_instruction,
            &[
                from_account.to_account_info(),
                to_account.clone(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[],
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct RequestForExecution<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: this is the recipient of the payment, the address of which is encoded in the quote and verified in the instruction
    #[account(mut)]
    pub payee: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RequestForExecutionArgs {
    pub amount: u64,
    pub dst_chain: u16,
    pub dst_addr: [u8; 32],
    pub refund_addr: Pubkey,
    pub signed_quote_bytes: Vec<u8>,
    pub request_bytes: Vec<u8>,
    pub relay_instructions: Vec<u8>,
}

#[error_code]
pub enum ExecutorErrors {
    #[msg("InvalidArguments")]
    InvalidArguments = 0x0,
    #[msg("QuoteSrcChainMismatch")]
    QuoteSrcChainMismatch = 0x1,
    #[msg("QuoteDstChainMismatch")]
    QuoteDstChainMismatch = 0x2,
    #[msg("QuoteExpired")]
    QuoteExpired = 0x3,
    #[msg("QuotePayeeMismatch")]
    QuotePayeeMismatch = 0x4,
}
