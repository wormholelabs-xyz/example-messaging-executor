use pinocchio::program_error::ProgramError;

/// Custom error codes for ExecutorQuoter program.
/// Error codes start at 0x1000 to avoid collision with built-in errors.
#[repr(u32)]
pub enum ExecutorQuoterError {
    /// Caller is not the authorized updater
    InvalidUpdater = 0x1000,
    /// Destination chain is not enabled
    ChainDisabled = 0x1001,
    /// Unsupported relay instruction type
    UnsupportedInstruction = 0x1002,
    /// Only one drop-off instruction is allowed
    MoreThanOneDropOff = 0x1003,
    /// Arithmetic overflow in quote calculation
    MathOverflow = 0x1004,
    /// Invalid relay instruction data
    InvalidRelayInstructions = 0x1005,
    /// Invalid PDA derivation
    InvalidPda = 0x1006,
    /// Account already initialized
    AlreadyInitialized = 0x1007,
    /// Account not initialized
    NotInitialized = 0x1008,
    /// Invalid account owner
    InvalidOwner = 0x1009,
    /// Invalid instruction data
    InvalidInstructionData = 0x100A,
    /// Invalid account discriminator
    InvalidDiscriminator = 0x100B,
}

impl From<ExecutorQuoterError> for ProgramError {
    fn from(e: ExecutorQuoterError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
