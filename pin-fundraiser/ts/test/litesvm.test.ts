// LiteSVM integration tests for the Pinocchio fundraiser program.
//
// LiteSVM runs the compiled SBF program in an in-process SVM (with the SPL
// Token and System programs bundled), so these tests exercise the real
// bytecode end-to-end, including CPIs and PDA signing — no validator needed.
//
// Run:  npm run test:litesvm   (build the program first: cargo build-sbf)

import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import {
  Keypair,
  PublicKey,
  Transaction,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { LiteSVM, FailedTransactionMetadata, TransactionMetadata } from "litesvm";

import {
  PROGRAM_ID,
  contributorPda,
  contributeIx,
  checkIx,
  fundraiserPda,
  initializeIx,
  refundIx,
  vaultPda,
} from "../src/client.js";
import {
  MINT_SIZE,
  TOKEN_ACCOUNT_SIZE,
  packMint,
  packTokenAccount,
  readTokenAmount,
} from "../src/spl.js";

const SO_PATH = fileURLToPath(
  new URL("../../target/deploy/pinocchio_fundraiser.so", import.meta.url),
);

const MINT_RENT = 1_461_600;
const TOKEN_ACC_RENT = 2_039_280;
const SOL = 1_000_000_000n;
const DECIMALS = 6;

function freshSvm(): LiteSVM {
  const svm = new LiteSVM();
  svm.addProgramFromFile(PROGRAM_ID, SO_PATH);
  return svm;
}

function setMint(svm: LiteSVM, mint: PublicKey, authority: PublicKey) {
  svm.setAccount(mint, {
    lamports: MINT_RENT,
    data: packMint(DECIMALS, authority),
    owner: TOKEN_PROGRAM_ID,
    executable: false,
    rentEpoch: 0,
  });
}

function setTokenAccount(
  svm: LiteSVM,
  address: PublicKey,
  mint: PublicKey,
  owner: PublicKey,
  amount: bigint | number,
) {
  svm.setAccount(address, {
    lamports: TOKEN_ACC_RENT,
    data: packTokenAccount(mint, owner, amount),
    owner: TOKEN_PROGRAM_ID,
    executable: false,
    rentEpoch: 0,
  });
}

function send(
  svm: LiteSVM,
  label: string,
  ix: Transaction["instructions"][number],
  payer: Keypair,
  signers: Keypair[],
): TransactionMetadata {
  const tx = new Transaction();
  tx.add(ix);
  tx.recentBlockhash = svm.latestBlockhash();
  tx.feePayer = payer.publicKey;
  tx.sign(...signers);
  const res = svm.sendTransaction(tx);
  if (res instanceof FailedTransactionMetadata) {
    console.error(res.meta().logs().join("\n"));
    throw new Error(`[${label}] tx failed: ${JSON.stringify(res.err())}`);
  }
  console.log(`  CU[${label}] = ${res.computeUnitsConsumed()}`);
  // litesvm does not auto-advance the blockhash; expire it to allow the next tx.
  svm.expireBlockhash();
  return res;
}

function readAmount(svm: LiteSVM, address: PublicKey): bigint {
  const acc = svm.getAccount(address);
  assert(acc, `account ${address.toBase58()} missing`);
  return readTokenAmount(acc!.data);
}

// ---------------------------------------------------------------------------

function testInitContributeRefund() {
  console.log("\n== LiteSVM: initialize / contribute / refund ==");
  const svm = freshSvm();

  const maker = Keypair.generate();
  const contributor = Keypair.generate();
  const mint = Keypair.generate().publicKey;
  svm.airdrop(maker.publicKey, 10n * SOL);
  svm.airdrop(contributor.publicKey, 10n * SOL);
  setMint(svm, mint, maker.publicKey);

  const [fundraiser, fundraiserBump] = fundraiserPda(maker.publicKey);
  const [vault, vaultBump] = vaultPda(fundraiser);
  const [cAccount, contributorBump] = contributorPda(fundraiser, contributor.publicKey);

  const contributorAta = getAssociatedTokenAddressSync(mint, contributor.publicKey);
  setTokenAccount(svm, contributorAta, mint, contributor.publicKey, 10_000_000);

  const amountToRaise = 30_000_000n;
  const contribution = 1_000_000n;

  send(
    svm,
    "initialize",
    initializeIx({
      maker: maker.publicKey,
      mint,
      fundraiser,
      vault,
      amount: amountToRaise,
      duration: 0,
      fundraiserBump,
      vaultBump,
    }),
    maker,
    [maker],
  );

  // Fundraiser state created (122 bytes, program-owned).
  const fr = svm.getAccount(fundraiser)!;
  assert.equal(fr.data.length, 122);
  assert(new PublicKey(fr.owner).equals(PROGRAM_ID));

  send(
    svm,
    "contribute",
    contributeIx({
      contributor: contributor.publicKey,
      fundraiser,
      contributorAccount: cAccount,
      contributorAta,
      vault,
      amount: contribution,
      contributorBump,
    }),
    contributor,
    [contributor],
  );

  assert.equal(readAmount(svm, vault), contribution, "vault funded");
  assert.equal(readAmount(svm, contributorAta), 9_000_000n, "contributor debited");

  send(
    svm,
    "refund",
    refundIx({
      contributor: contributor.publicKey,
      maker: maker.publicKey,
      fundraiser,
      contributorAccount: cAccount,
      contributorAta,
      vault,
      contributorBump,
    }),
    contributor,
    [contributor],
  );

  assert.equal(readAmount(svm, contributorAta), 10_000_000n, "contributor refunded");
  const closed = svm.getAccount(cAccount);
  assert(!closed || closed.data.length === 0, "contributor record closed");
  console.log("  ✓ init/contribute/refund OK");
}

function testCheckReleasesToMaker() {
  console.log("\n== LiteSVM: initialize / check (target met) ==");
  const svm = freshSvm();

  const maker = Keypair.generate();
  const mint = Keypair.generate().publicKey;
  svm.airdrop(maker.publicKey, 10n * SOL);
  setMint(svm, mint, maker.publicKey);

  const [fundraiser, fundraiserBump] = fundraiserPda(maker.publicKey);
  const [vault, vaultBump] = vaultPda(fundraiser);
  const makerAta = getAssociatedTokenAddressSync(mint, maker.publicKey);
  setTokenAccount(svm, makerAta, mint, maker.publicKey, 0);

  const amountToRaise = 5_000_000n;

  send(
    svm,
    "initialize",
    initializeIx({
      maker: maker.publicKey,
      mint,
      fundraiser,
      vault,
      amount: amountToRaise,
      duration: 0,
      fundraiserBump,
      vaultBump,
    }),
    maker,
    [maker],
  );

  // Simulate a fully-funded vault.
  setTokenAccount(svm, vault, mint, fundraiser, amountToRaise);

  send(
    svm,
    "check",
    checkIx({ maker: maker.publicKey, fundraiser, vault, makerAta }),
    maker,
    [maker],
  );

  assert.equal(readAmount(svm, makerAta), amountToRaise, "maker received funds");
  const fr = svm.getAccount(fundraiser);
  assert(!fr || fr.data.length === 0, "fundraiser closed");
  console.log("  ✓ init/check OK");
}

testInitContributeRefund();
testCheckReleasesToMaker();
console.log("\nAll LiteSVM tests passed.\n");
