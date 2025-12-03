use anchor_lang::prelude::*;

/// Custom error codes for ExecutorQuoter program.
/// Error codes start at 0x1000 to match the Pinocchio implementation.
#[error_code]
pub enum ExecutorQuoterError {
    /// Caller is not the authorized updater
    #[msg("Caller is not the authorized updater")]
    InvalidUpdater = 0x1000,

    /// Destination chain is not enabled
    #[msg("Destination chain is not enabled")]
    ChainDisabled = 0x1001,

    /// Unsupported relay instruction type
    #[msg("Unsupported relay instruction type")]
    UnsupportedInstruction = 0x1002,

    /// Only one drop-off instruction is allowed
    #[msg("Only one drop-off instruction is allowed")]
    MoreThanOneDropOff = 0x1003,

    /// Arithmetic overflow in quote calculation
    #[msg("Arithmetic overflow in quote calculation")]
    MathOverflow = 0x1004,

    /// Invalid relay instruction data
    #[msg("Invalid relay instruction data")]
    InvalidRelayInstructions = 0x1005,

    /// Invalid PDA derivation
    #[msg("Invalid PDA derivation")]
    InvalidPda = 0x1006,

    /// Account already initialized
    #[msg("Account already initialized")]
    AlreadyInitialized = 0x1007,

    /// Account not initialized
    #[msg("Account not initialized")]
    NotInitialized = 0x1008,

    /// Invalid account owner
    #[msg("Invalid account owner")]
    InvalidOwner = 0x1009,

    /// Invalid instruction data
    #[msg("Invalid instruction data")]
    InvalidInstructionData = 0x100A,

    /// Invalid account discriminator
    #[msg("Invalid account discriminator")]
    InvalidDiscriminator = 0x100B,
}
