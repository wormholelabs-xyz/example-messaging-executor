# Executor Quoter

Example quoter program for the Wormhole executor system. This implementation serves as a reference for integrators building their own quoter logic.

## Interface Specification

The quoter interface is defined by CPI calls from the `executor-quoter-router` program. Integrators must implement two required instructions with specific discriminators:

### Required Instructions

**RequestQuote (discriminator: 2)**

- Returns a quote for cross-chain execution
- Accounts: `[config, chain_info, quote_body]`
- Returns: `u64` payment amount (8 bytes, big-endian)

**RequestExecutionQuote (discriminator: 3)**

- Returns full execution details including payment, payee, and quote body
- Accounts: `[config, chain_info, quote_body, event_cpi]`
- Returns: 72 bytes (`u64` payment + 32-byte payee address + 32-byte quote body)

Both instructions support up to 8-byte discriminators for Anchor compatibility (byte 0 = instruction ID, bytes 1-7 = padding zeros).

### Instruction Data Layout

Both instructions share the same input format (after discriminator):

- `dst_chain`: u16 (LE)
- `dst_addr`: [u8; 32]
- `refund_addr`: [u8; 32]
- `request_bytes_len`: u32 (LE)
- `request_bytes`: variable
- `relay_instructions_len`: u32 (LE)
- `relay_instructions`: variable

This is compatible with Borsh serialization:

```rust
#[derive(borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct RequestQuoteData {
    pub dst_chain: u16,
    pub dst_addr: [u8; 32],
    pub refund_addr: [u8; 32],
    pub request_bytes: Vec<u8>,
    pub relay_instructions: Vec<u8>,
}
```

## Optional Instructions

The following instructions have no inherent spec and are left to integrator discretion:

- **UpdateChainInfo (discriminator: 0)**: Configure per-chain parameters
- **UpdateQuote (discriminator: 1)**: Update pricing data
- Any additional instructions one may wish to add

This example uses 1-byte discriminators for admin instructions, but this is by no means necessary.

## Reserved Accounts

- **config**: First account in CPI calls. Unused in this example but available for integrator-specific configuration state.
- **event_cpi**: Fourth account in `RequestExecutionQuote`. Reserved for integrators who want to emit events during quote execution.
