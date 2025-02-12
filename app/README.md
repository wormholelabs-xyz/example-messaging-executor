# Off-Chain Executor Service

## Integration

> 🚧 The off-chain Executor service is under active development and subject to change!

### Quote

#### Request

GET `/v0/quote/:srcChain/:dstChain`

##### Example

http://localhost:3000/v0/quote/1/6

#### Response

```typescript
{
  signedQuote: `0x${string}`;
}
```

### Estimate

#### Request

GET `/v0/estimate/:quote/:relayInstructions`

##### Example

http://localhost:3000/v0/estimate/0x455130318f26a0025dccc6cfc07a7d38756280a10e295ad783718b7ec89617b7040685e01bdcca03214022980daae91340e0c3f840c005ef000100060000000067accbbf00000000000003e8000000003b9aca01000001c0ebb731000000003a3b1f25001c2af602e16e0759010a636057216e8e1759dea0b8eefa33389828dbc5fdac435e08fa02542ed6cb337f0af86b43b1502468370547d8476403f3b49a47cc05c11c/0x010000000000000000000000000003d09000000000000000000000000000000000

#### Response

```typescript
{
    quote: SignedQuote,
    estimate: string
}
```

### Request For Execution

#### Request

GET `/v0/request/VAAv1/:chain/:emitter/:sequence`

##### Example

http://localhost:3000/v0/request/VAAv1/30/000000000000000000000000706f82e9bb5b0813501714ab5974216704980e31/137279

#### Response

```typescript
{
  bytes: `0x${string}`;
}
```

### Status

Fetching status also kicks off the relay.

#### Request

GET `/v0/status/:id`

##### Example

SVM - http://localhost:3000/v0/status/00011bec45bbd344eb129ca620ed4cf59ecd56ad25413fc8b5db1aa51029028986308da4bdb4560e12d8a363132c0bb8bf86459da3dbd7b444f4895221165b826d0b

EVM - http://localhost:3000/v0/status/0002f80e39f3163f679737deef86527ef9372f5d54abfe5cfc509fc9c529d6aa36ea0000000000000000000000000000000000000000000000000000000000000007

#### Response

```typescript
{
    status: string;
    requestForExecution: RequestForExecution;
    txs: string[];
    instruction?: VAAv1Request | ModularMessageRequest;
}
```

## Run

Install dependencies with `bun install --frozen-lockfile`.

Create a `.env` file with the following key/value pairs.

```bash
QUOTER_KEY=0x<privateKeyHex>
ETH_KEY=0x<privateKeyHex>
SOL_KEY=0x<privateKeyHex>
GUARDIAN_URL=https://api.testnet.wormholescan.io
```

Run with `bun run start` or `bun run dev` to watch for changes.

The following environment variables are supported.

- `QUOTER_KEY` - The private key used to sign quotes for this service. This is only for off-chain use and does not need to custody any funds.
- `ETH_KEY` - The private key used for relaying to EVM destination chains. It requires funds on every supported EVM destination chain.
- `SOL_KEY` - The private key used for relaying to SVM destination chains. It requires funds on every supported SVM destination chain.
- `GUARDIAN_URL` - The base URL used for fetching v1 VAAs.
- `LOG_LEVEL` - The [winston log level](https://github.com/winstonjs/winston?tab=readme-ov-file#logging-levels) to use. Defaults to `info`.
- `PORT` - The port for the express server to use. Defaults to `3000`
