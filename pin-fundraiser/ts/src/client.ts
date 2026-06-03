// Minimal client for the Pinocchio fundraiser program.
//
// The program has no Anchor IDL, so we hand-encode the four instructions.
// This module is shared by the LiteSVM tests and the devnet runner.

import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";

// Defaults to the program ID baked into the source; override on devnet/mainnet
// with the address you deployed to (the deploy script keeps the on-chain
// `crate::ID` in sync with this).
export const PROGRAM_ID = new PublicKey(
  process.env.FUNDRAISER_PROGRAM_ID ??
    "HeaBbw9V4mTWhMXrT2EB6W3EdZXgZrW2fm3Kq3CTUsLt",
);

// Instruction discriminators (first byte of the instruction data).
const IX = { initialize: 0, contribute: 1, check: 2, refund: 3 } as const;

function u64le(v: bigint | number): Buffer {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(BigInt(v));
  return b;
}

// --- PDA derivations ---------------------------------------------------------

export function fundraiserPda(maker: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("fundraiser"), maker.toBuffer()],
    PROGRAM_ID,
  );
}

export function vaultPda(fundraiser: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), fundraiser.toBuffer()],
    PROGRAM_ID,
  );
}

export function contributorPda(
  fundraiser: PublicKey,
  contributor: PublicKey,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("contributor"), fundraiser.toBuffer(), contributor.toBuffer()],
    PROGRAM_ID,
  );
}

// --- instruction builders ----------------------------------------------------

export function initializeIx(args: {
  maker: PublicKey;
  mint: PublicKey;
  fundraiser: PublicKey;
  vault: PublicKey;
  amount: bigint | number;
  duration: number;
  fundraiserBump: number;
  vaultBump: number;
}): TransactionInstruction {
  const data = Buffer.concat([
    Buffer.from([IX.initialize]),
    u64le(args.amount),
    Buffer.from([args.duration, args.fundraiserBump, args.vaultBump]),
  ]);
  return new TransactionInstruction({
    programId: PROGRAM_ID,
    data,
    keys: [
      { pubkey: args.maker, isSigner: true, isWritable: true },
      { pubkey: args.mint, isSigner: false, isWritable: false },
      { pubkey: args.fundraiser, isSigner: false, isWritable: true },
      { pubkey: args.vault, isSigner: false, isWritable: true },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
  });
}

export function contributeIx(args: {
  contributor: PublicKey;
  fundraiser: PublicKey;
  contributorAccount: PublicKey;
  contributorAta: PublicKey;
  vault: PublicKey;
  amount: bigint | number;
  contributorBump: number;
}): TransactionInstruction {
  const data = Buffer.concat([
    Buffer.from([IX.contribute]),
    u64le(args.amount),
    Buffer.from([args.contributorBump]),
  ]);
  return new TransactionInstruction({
    programId: PROGRAM_ID,
    data,
    keys: [
      { pubkey: args.contributor, isSigner: true, isWritable: true },
      { pubkey: args.fundraiser, isSigner: false, isWritable: true },
      { pubkey: args.contributorAccount, isSigner: false, isWritable: true },
      { pubkey: args.contributorAta, isSigner: false, isWritable: true },
      { pubkey: args.vault, isSigner: false, isWritable: true },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
  });
}

export function checkIx(args: {
  maker: PublicKey;
  fundraiser: PublicKey;
  vault: PublicKey;
  makerAta: PublicKey;
}): TransactionInstruction {
  return new TransactionInstruction({
    programId: PROGRAM_ID,
    data: Buffer.from([IX.check]),
    keys: [
      { pubkey: args.maker, isSigner: true, isWritable: true },
      { pubkey: args.fundraiser, isSigner: false, isWritable: true },
      { pubkey: args.vault, isSigner: false, isWritable: true },
      { pubkey: args.makerAta, isSigner: false, isWritable: true },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ],
  });
}

export function refundIx(args: {
  contributor: PublicKey;
  maker: PublicKey;
  fundraiser: PublicKey;
  contributorAccount: PublicKey;
  contributorAta: PublicKey;
  vault: PublicKey;
  contributorBump: number;
}): TransactionInstruction {
  return new TransactionInstruction({
    programId: PROGRAM_ID,
    data: Buffer.from([IX.refund, args.contributorBump]),
    keys: [
      { pubkey: args.contributor, isSigner: true, isWritable: true },
      { pubkey: args.maker, isSigner: false, isWritable: false },
      { pubkey: args.fundraiser, isSigner: false, isWritable: true },
      { pubkey: args.contributorAccount, isSigner: false, isWritable: true },
      { pubkey: args.contributorAta, isSigner: false, isWritable: true },
      { pubkey: args.vault, isSigner: false, isWritable: true },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    ],
  });
}
