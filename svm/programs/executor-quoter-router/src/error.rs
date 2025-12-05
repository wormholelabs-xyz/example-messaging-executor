use pinocchio::program_error::ProgramError;

/// Custom errors for the ExecutorQuoterRouter program.
#[repr(u32)]
pub enum ExecutorQuoterRouterError {
    /// Invalid account owner
    InvalidOwner = 0,
    /// Invalid account discriminator
    InvalidDiscriminator = 1,
    /// Invalid governance message prefix
    InvalidGovernancePrefix = 2,
    /// Chain ID mismatch
    ChainIdMismatch = 3,
    /// Invalid sender - msg.sender does not match universal_sender_address
    InvalidSender = 4,
    /// Governance message has expired
    GovernanceExpired = 5,
    /// Invalid signature - ecrecover failed or signer mismatch
    InvalidSignature = 6,
    /// Universal address is not a valid EVM address (upper 12 bytes non-zero)
    NotAnEvmAddress = 7,
    /// Quoter not registered
    QuoterNotRegistered = 8,
    /// Underpaid - payment less than required
    Underpaid = 9,
    /// Refund failed
    RefundFailed = 10,
    /// Invalid instruction data
    InvalidInstructionData = 11,
    /// CPI failed
    CpiFailed = 12,
    /// Invalid return data from quoter
    InvalidReturnData = 13,
    /// Math overflow
    MathOverflow = 14,
}

impl From<ExecutorQuoterRouterError> for ProgramError {
    fn from(e: ExecutorQuoterRouterError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
