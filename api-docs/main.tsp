import "@typespec/http";
import "@typespec/openapi";

using Http;
using OpenAPI;

@service()
@info(#{
  title: "Messaging Executor API",
  version: "1.0.0",
})
@server("http://localhost:3000/v0", "Local development server")
namespace MessagingExecutor;

// ============= Models =============

model Capabilities {
  @doc("Request prefixes enabled for given chain")
  requestPrefixes: string[];

  @doc("Gas Drop-off limit, native chain token bigint represented as string") 
  gasDropOffLimit?: string;

  @doc("Maximum Gas limit") 
  maxGasLimit?: string;

  @doc("Max message value, native chain token bigint represented as string") 
  maxMsgValue?: string;
}

@doc("Object with chain IDs (Wormhole Chain ID) as keys and their capabilities as values")
model CapabilitiesResponse is Record<Capabilities>;

model QuoteRequest {
  @doc("Source wormhole chain ID")
  srcChain: int32;
  
  @doc("Destination wormhole chain ID")
  dstChain: int32;
  
  @doc("Optional hex-encoded relay instructions for cost estimation")
  relayInstructions?: string;
}

model QuoteResponse {
  @doc("Hex-encoded signed quote")
  signedQuote: string;
  
  @doc("Estimated cost in source chain native token units")
  estimatedCost?: string;
}

model StatusRequest {
  @doc("Wormhole chain ID for the transaction")
  chainId?: int32;
  
  @doc("Transaction hash")
  txHash: string;
}

model TxInfo {
  @doc("Relayed transaction hash")
  txHash: string;

  @doc("Wormhole chain ID for the transaction")
  chainId: int32;

  @doc("Block number (or slot/epoch)")
  blockNumber: string;

  @doc("Block time as timestamp")
  blockTime?: string;

   @doc("Relay cost in native token units (wei/lamports/etc)")
  cost: string;
}

model RequestForExecution {
  @doc("Public key of Quoter")
  quoterAddress: string;
  
  @doc("Amount paid for the relay in source chain native token")
  amtPaid: string;
  
  @doc("Destination Wormhole chain ID")
  dstChain: int32;
  
  @doc("Destination address to execute the transaction")
  dstAddr: string;
  
  @doc("Refund address for excess payment")
  refundAddr: string;
  
  @doc("Hex-encoded signed quote bytes")
  signedQuoteBytes: string;
  
  @doc("Hex-encoded request bytes")
  requestBytes: string;
  
  @doc("Hex-encoded relay instruction bytes")
  relayInstructionsBytes: string;
  
  @doc("Timestamp when the request was created")
  timestamp: string;
}

model SignedQuote {
  @doc("Quote details")
  quote: {
    @doc("Quote prefix identifier (e.g., 'EQ01')")
    prefix: string;
    
    @doc("Address of the quoter providing the quote")
    quoterAddress: string;
    
    @doc("Address that will receive payment")
    payeeAddress: string;
    
    @doc("Source Wormhole chain ID")
    srcChain: int32;
    
    @doc("Destination Wormhole chain ID")
    dstChain: int32;
    
    @doc("Quote expiration time")
    expiryTime: string;
    
    @doc("Base fee for the relay")
    baseFee: string;
    
    @doc("Gas price on destination chain")
    dstGasPrice: string;
    
    @doc("Source chain token price")
    srcPrice: string;
    
    @doc("Destination chain token price")
    dstPrice: string;
  };
  
  @doc("Cryptographic signature of the quote")
  signature?: string;
}

// Request instruction variants based on prefix
model ERV1Request {
  @doc("Request prefix identifier")
  prefix: "ERV1";
  
  @doc("Chain ID")
  chain: int32;
  
  @doc("Contract address")
  address: string;
  
  @doc("Sequence number")
  sequence: string;
}

model ERN1Request {
  @doc("Request prefix identifier")
  prefix: "ERN1";
  
  @doc("Source chain ID")
  srcChain: int32;
  
  @doc("Source manager address")
  srcManager: string;
  
  @doc("Message identifier")
  messageId: string;
}

model ERC1Request {
  @doc("Request prefix identifier")
  prefix: "ERC1";
  
  @doc("Source domain")
  sourceDomain: int32;
  
  @doc("Nonce value")
  nonce: string;
}

model ERC2Request {
  @doc("Request prefix identifier")
  prefix: "ERC2";
  
  @doc("CCTP V2 request configuration")
  cctpV2Request: {
    @doc("CCTP V2 request prefix mode")
    cctpV2RequestPrefix: string;
  };
}

@doc("Request instruction details that vary by prefix type")
model RequestInstruction {
  @doc("The request details, structure depends on the prefix type")
  request: ERV1Request | ERN1Request | ERC1Request | ERC2Request;
}

model RelayTransaction {
  @doc("Wormhole chain ID where the transaction originated")
  chainId: int32;
  
  @doc("Estimated cost for the relay in source chain native token")
  estimatedCost: string;
  
  @doc("Unique identifier for the relay transaction")
  id: string;
  
  @doc("Timestamp when the transaction was indexed")
  indexedAt: string;
  
  @doc("Parsed relay instruction details")
  instruction?: RequestInstruction;
  
  @doc("Original request for execution details")
  requestForExecution: RequestForExecution;
  
  @doc("Signed quote used for this transaction")
  signedQuote: SignedQuote;
  
  @doc("Current status of the relay (pending, submitted, failed, etc.)")
  status: string;
  
  @doc("Original transaction hash")
  txHash: string;
  
  @doc("Reason for failure if status is 'failed'")
  failureCause?: string;
  
  @doc("List of destination chain transactions executed")
  txs?: TxInfo[];
}

@error
model ErrorResponse {
  message: string;
  statusCode?: int32;
}

// ============= Interfaces =============

@route("/capabilities")
@tag("Capabilities")
interface CapabilitiesInterface {
  @summary("Get capabilities for all supported chains")
  @doc("Retrieves the supported features and capabilities for all supported blockchains (as wormhole chain ID)")
  @get
  list(): CapabilitiesResponse | ErrorResponse;
}

@route("/quote")
@tag("Quote")
interface QuoteInterface {
  @summary("Generate a quote for cross-chain transaction")
  @doc("Provides a signed quote for cross-chain relay with optional cost estimation")
  @post
  create(@body body: QuoteRequest): QuoteResponse | ErrorResponse;
}

@route("/status")
@tag("Status")
interface StatusInterface {
  @summary("Get status of a transaction")
  @doc("Retrieves the status of a request for execution transaction and potentially initiates processing (when first time statusing and chain ID provided)")
  @route("tx")
  @post
  getTransaction(@body body: StatusRequest): RelayTransaction[] | ErrorResponse;
}
