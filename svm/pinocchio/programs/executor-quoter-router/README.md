# Executor Quoter Router

Router program that dispatches quote requests and execution requests to registered quoter implementations.

## Overview

The router manages quoter registrations and routes CPI calls to the appropriate quoter program. It defines the interface that quoter implementations must adhere to.

## Instructions

**UpdateQuoterContract (discriminator: 0)**

- Registers or updates a quoter's implementation mapping
- Accounts: `[payer, sender, config, quoter_registration, system_program]`

**QuoteExecution (discriminator: 1)**

- Gets a quote from a registered quoter via CPI
- Accounts: `[quoter_registration, quoter_program, config, chain_info, quote_body]`
- CPI to quoter's `RequestQuote` instruction (discriminator: `[2, 0, 0, 0, 0, 0, 0, 0]`)

**RequestExecution (discriminator: 2)**

- Executes cross-chain request through the router
- Accounts: `[payer, config, quoter_registration, quoter_program, executor_program, payee, refund_addr, system_program, quoter_config, chain_info, quote_body, event_cpi]`
- CPI to quoter's `RequestExecutionQuote` instruction (discriminator: `[3, 0, 0, 0, 0, 0, 0, 0]`)

## Quoter Interface Requirements

Quoter implementations must support the following CPI interface:

### RequestQuote

- Discriminator: 8 bytes (`[2, 0, 0, 0, 0, 0, 0, 0]`)
- Accounts: `[config, chain_info, quote_body]`
- Returns: `u64` (big endian) payment amount via `set_return_data`

### RequestExecutionQuote

- Discriminator: 8 bytes (`[3, 0, 0, 0, 0, 0, 0, 0]`)
- Accounts: `[config, chain_info, quote_body, event_cpi]`
- Returns: 72 bytes via `set_return_data`:
  - bytes 0-7: `u64` required payment (big-endian)
  - bytes 8-39: 32-byte payee address
  - bytes 40-71: 32-byte quote body (EQ01 format)

See `executor-quoter` for a reference implementation.
