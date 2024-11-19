export type RequestForExecution = {
  quoterAddress: string;
  amtPaid: bigint;
  dstChain: number;
  dstAddr: string;
  gasLimit: bigint;
  msgValue: bigint;
  refundAddr: string;
  signedQuoteBytes: string;
  requestBytes: string;
};
