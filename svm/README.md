# Executor on Solana

## Integration

> 🚧 Solana integration is under active development and subject to change!

Integrating with Executor on Solana involves two aspects, requesting execution via the Executor program and being executed by an off-chain relayer service.

### Requesting Execution

In order to request execution, you need the following information:

```rust
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
    pub amount: u64,                 // Amount to pay the payee for execution
    pub dst_chain: u16,              // Wormhole Chain ID of the destination chain
    pub dst_addr: [u8; 32],          // UniversalAddress of the destination contract to execute
    pub refund_addr: Pubkey,         // Native address to refund excess payment to
    pub signed_quote_bytes: Vec<u8>,
    pub request_bytes: Vec<u8>,
    pub relay_instructions: Vec<u8>,
}
```

With that you can invoke the [`request_for_execution`](./programs/executor/src/lib.rs) instruction which performs limited validation on the signed quote and pays the designated payee the specified amount.

See the [design](../README.md) for more details on:

- [Signed Quote](../README.md#off-chain-quote)
- [Request For Execution](../README.md#request-for-execution)
- [Relay Instructions](../README.md#relay-instructions)

For ease of integration and flexibility, it is encouraged to pass in `relay_instructions` from off-chain.

The IDL for the Executor program can be built by running `anchor build` in this folder.

Copy this into the `idls` directory of your Anchor project in order to leverage Anchor's [dependency free composability](https://www.anchor-lang.com/docs/features/declare-program).

#### Example v1 VAA Request

For example, after invoking an instruction which publishes a Wormhole Core message, fetch the sequence number for the emitter and request execution.

```rust
declare_program!(executor);
use executor::{program::Executor, types::RequestForExecutionArgs};
use wormhole_anchor_sdk;

const REQ_VAA_V1: &[u8; 4] = b"ERV1";

// NOTE: this is not yet available in a published crate
pub fn make_vaa_v1_request(chain: u16, address: [u8; 32], sequence: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity({
        4 // type
        + 2 // chain
        + 32 // address
        + 8 // sequence
    });
    out.extend_from_slice(REQ_VAA_V1);
    out.extend_from_slice(&chain.to_be_bytes());
    out.extend_from_slice(&address);
    out.extend_from_slice(&sequence.to_be_bytes());
    out
}

...

// parse the sequence from the account and request execution
// reading the account after avoids having to handle when the account doesn't exist
let mut buf = &ctx.accounts.emitter_sequence.try_borrow_mut_data()?[..];
let seq = wormhole_anchor_sdk::wormhole::SequenceTracker::try_deserialize(&mut buf)?;
executor::cpi::request_for_execution(
    CpiContext::new(
        ctx.accounts.executor_program.to_account_info(),
        executor::cpi::accounts::RequestForExecution {
            payer: ctx.accounts.payer.to_account_info(),
            payee: ctx.accounts.payee.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
    ),
    RequestForExecutionArgs {
        amount: exec_amount,
        dst_chain: recipient_chain,
        dst_addr: dst_execution_address,
        refund_addr: ctx.accounts.payer.key(),
        signed_quote_bytes,
        request_bytes: make_vaa_v1_request(
            OUR_CHAIN,
            ctx.accounts.emitter.key().to_bytes(),
            seq.sequence - 1,
        ),
        relay_instructions,
    },
)
```

### Execution Support

Due to the requirements of specifying accounts for Solana instructions and the potential need to perform multiple instructions to complete an execution, the Solana Executor off-chain implementation performs multiple steps, based on the type of execution, to build a transaction according to the integrating program's specifications.

The `Ix` and `AcctMeta` structs are used to facilitate this, which essentially mirror `solana_program`'s [`Instruction`](https://docs.rs/solana-program/2.1.13/solana_program/instruction/struct.Instruction.html) and [`AccountMeta`](https://docs.rs/solana-program/2.1.13/solana_program/instruction/struct.AccountMeta.html).

```rust
#[derive(AnchorSerialize)]
pub struct AcctMeta {
    /// An account's public key.
    pub pubkey: Pubkey,
    /// True if an `Instruction` requires a `Transaction` signature matching `pubkey`.
    pub is_signer: bool,
    /// True if the account data or metadata may be mutated during program execution.
    pub is_writable: bool,
}

#[derive(AnchorSerialize)]
pub struct Ix {
    /// Pubkey of the program that executes this instruction.
    pub program_id: Pubkey,
    /// Metadata describing accounts that should be passed to the program.
    pub accounts: Vec<AcctMeta>,
    /// Opaque data passed to the program for its own interpretation.
    pub data: Vec<u8>,
}
```

Certain special pubkey bytes will be replaced by the off-chain relayer, per the spec.

```rust
const PAYER: &[u8; 32] = b"payer000000000000000000000000000";
```

#### v1 VAA Execution

1. The off-chain relayer will first call `execute_vaa_v1` on the designated program with the VAA body, which must return an `Ix`.
2. It will then determine if the [`PostedVAA`](https://github.com/wormholelabs-xyz/wormhole/blob/39f4d6e94bb41e47d9df0607c5dd6d8ae846df19/solana/bridge/program/src/accounts/posted_vaa.rs#L25-L29) account for the Core Bridge program is required, and if so, post the VAA.
3. Lastly, it will invoke the designated instruction.

<!-- cspell:disable -->

```rust
use wormhole_raw_vaas::Body;
pub fn execute_vaa_v1(_ctx: Context<ExecuteVaaV1>, vaa_body: Vec<u8>) -> Result<Ix> {
    // Compute the message hash.
    let message_hash = solana_program::keccak::hashv(&[&vaa_body]).to_bytes();
    // Parse the body.
    let body = Body::parse(&vaa_body).map_err(|_| ExampleError::FailedToParseVaaBody)?;
    // Calculate accounts
    ...
    // Build instruction
    Ok(Ix {
            program_id: crate::ID,
            data: data.data(),
            accounts: vec![
                AcctMeta {
                    pubkey: Pubkey::from(*PAYER),
                    is_writable: true,
                    is_signer: true,
                },
                ...
            ]
    })
```

<!-- cspell:enable -->

## Executor Development

This folder was generated using [Anchor](https://www.anchor-lang.com/) with `anchor init --no-git`.

### Testing

```bash
anchor test
```

### Building

```bash
anchor build
```

For verifiable builds use

```bash
anchor build --verifiable
```

### Deploying

#### Solana Devnet

```bash
anchor deploy --provider.cluster devnet --provider.wallet ~/.config/solana/your-key.json
```

#### Mainnet

```bash
anchor deploy --provider.cluster mainnet --provider.wallet ~/.config/solana/your-key.json
```

#### Upgrading

```
anchor upgrade --provider.cluster <network> --provider.wallet ~/.config/solana/your-key.json --program-id <PROGRAM_ID> target/deploy/svm.so
```

If you get an error like this

```
Error: Deploying program failed: RPC response error -32002: Transaction simulation failed: Error processing Instruction 0: account data too small for instruction [3 log messages]
```

Don't fret! Just extend the program size.

```
solana program -u <network> -k ~/.config/solana/your-key.json extend <PROGRAM_ID> <ADDITIONAL_BYTES>
```

You can view the current program size with `solana program -u <network> show <PROGRAM_ID>`.
