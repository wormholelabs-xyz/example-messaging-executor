import axios from "axios";
import {
  ModularMessageRequest,
  RequestForExecution,
  VAAv1Request,
} from "./requestForExecution";
import { CHAIN_TO_INFO, envStringRequired } from "./chainInfo";

const GUARDIAN_URL = envStringRequired("GUARDIAN_URL");

export async function relayVAAv1(r: RequestForExecution, v: VAAv1Request) {
  const vaaId = `${v.chain}/${v.address.slice(2)}/${v.sequence.toString()}`;
  const bytes = (await axios.get(`${GUARDIAN_URL}/v1/signed_vaa/${vaaId}`)).data
    ?.vaaBytes;
  if (!bytes) {
    throw new Error(`unable to fetch VAA ${vaaId}`);
  }
  const dstInfo = CHAIN_TO_INFO[r.dstChain];
  return dstInfo.handler.relayVAAv1(dstInfo, r, v, bytes);
}
export async function relayMM(
  r: RequestForExecution,
  m: ModularMessageRequest,
) {
  const dstInfo = CHAIN_TO_INFO[r.dstChain];
  return dstInfo.handler.relayMM(dstInfo, r, m);
}
