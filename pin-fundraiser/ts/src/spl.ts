// Helpers to hand-pack SPL Token `Mint` and `Account` state, so the LiteSVM
// tests can inject pre-funded token accounts directly via `setAccount`
// (no need to run the token program's init instructions).

import { PublicKey } from "@solana/web3.js";

export const MINT_SIZE = 82;
export const TOKEN_ACCOUNT_SIZE = 165;

export function packMint(decimals: number, mintAuthority: PublicKey): Buffer {
  const data = Buffer.alloc(MINT_SIZE);
  data.writeUInt32LE(1, 0); // mint_authority: COption::Some
  mintAuthority.toBuffer().copy(data, 4);
  // supply (36..44) = 0
  data.writeUInt8(decimals, 44);
  data.writeUInt8(1, 45); // is_initialized
  // freeze_authority: COption::None (46..82) = 0
  return data;
}

export function packTokenAccount(
  mint: PublicKey,
  owner: PublicKey,
  amount: bigint | number,
): Buffer {
  const data = Buffer.alloc(TOKEN_ACCOUNT_SIZE);
  mint.toBuffer().copy(data, 0);
  owner.toBuffer().copy(data, 32);
  data.writeBigUInt64LE(BigInt(amount), 64);
  // delegate: COption::None (72..108)
  data.writeUInt8(1, 108); // state = Initialized
  // is_native / delegated_amount / close_authority = 0
  return data;
}

export function readTokenAmount(data: Uint8Array): bigint {
  return Buffer.from(data).readBigUInt64LE(64);
}
