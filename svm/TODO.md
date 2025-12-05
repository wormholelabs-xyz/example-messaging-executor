# SVM Executor Programs - Remaining Tasks

## executor-quoter-router

### Events
- [ ] Emit `QuoterContractUpdate` event in `update_quoter_contract.rs`
- [ ] Emit `OnChainQuote` event in `request_execution.rs`

### Integration Tests
- [ ] `test_quote_execution` - basic CPI to quoter
- [ ] `test_request_execution` - full execution flow
- [ ] `test_request_execution_underpaid` - payment validation
- [ ] `test_request_execution_pays_payee` - payment routing
- [ ] + Any remaining tests for full coverage if it had forge (just mirror tests in the evm impl)

## executor-quoter

### Features
- [ ] Implement batch updates for executor-quoter program
