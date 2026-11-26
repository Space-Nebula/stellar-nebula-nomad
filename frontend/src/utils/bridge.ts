/** Client-side mirrors of the bridge contract rules (src/bridge/stellar_ethereum.rs, #713). */

export const BRIDGE_FEE_BPS = 30;
export const MAX_BRIDGE_FEE_BPS = 100;
export const MAX_BRIDGE_AMOUNT = 1_000_000_000_000_000;

export type BridgeStatus = 'Pending' | 'Confirmed' | 'Completed' | 'Failed' | 'Cancelled';
export type BridgeDirection = 'StellarToEth' | 'EthToStellar';

/** A `BridgeRequest` as read from `bridge_get_request`, amounts in the token's base units. */
export interface BridgeRequestView {
  requestId: number;
  asset: string;
  chainId: number;
  amount: number;
  fee: number;
  direction: BridgeDirection;
  status: BridgeStatus;
  /** Ethereum recipient (outbound) or sender (inbound), 0x-prefixed. */
  ethAddress: string;
}

/** Fee the contract takes: floor(amount * feeBps / 10_000), computed without float rounding. */
export function bridgeFee(amount: number, feeBps: number = BRIDGE_FEE_BPS): number {
  return Number((BigInt(amount) * BigInt(feeBps)) / 10_000n);
}

/** A 20-byte hex address that is not the zero address, as the contract requires. */
export function isValidEthAddress(address: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(address) && !/^0x0{40}$/.test(address);
}

/** Why `amount` would be rejected, or null when the contract would accept it. */
export function amountError(amount: number, maxAmount: number, balance: number): string | null {
  if (!Number.isSafeInteger(amount) || amount <= 0) return 'Enter a whole amount above zero.';
  if (amount > maxAmount) return `The bridge limit is ${maxAmount.toLocaleString('en-US')} per transfer.`;
  if (amount > balance) return 'The amount is more than your balance.';
  return null;
}

export const STATUS_TEXT: Record<BridgeStatus, string> = {
  Pending: 'Waiting for Ethereum',
  Confirmed: 'Minted on Ethereum',
  Completed: 'Completed',
  Failed: 'Failed (refunded)',
  Cancelled: 'Cancelled (refunded)',
};
