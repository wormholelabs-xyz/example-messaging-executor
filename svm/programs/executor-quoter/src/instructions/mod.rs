//! Instruction handlers for the ExecutorQuoter program.
//!
//! TODO: Add batch update instructions to match EVM's `quoteUpdate(Update[])` and
//! `chainInfoUpdate(Update[])` for updating multiple chains in a single transaction.

pub mod get_quote;
pub mod initialize;
pub mod update_chain_info;
pub mod update_quote;
