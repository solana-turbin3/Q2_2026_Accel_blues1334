// Devnet smoke test for the Pinocchio fundraiser program.
//
// Runs the full initialize -> contribute -> refund flow against a *live*
// devnet deployment, using a real SPL mint and ATA.
//
// Prerequisites:
//   1. Deploy the program:  ./devnet/deploy.sh         (prints the program id)
//   2. export FUNDRAISER_PROGRAM_ID=<printed id>
//   3. A funded payer keypair (default ~/.config/solana/id.json, or set
//      SOLANA_KEYPAIR=/path/to/keypair.json). Needs ~0.1 devnet SOL.
//
// Run:  npm run devnet

import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import {
  clusterApiUrl,
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  sendAndConfirmTransaction,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotent,
  createMint,
  getAccount,
  mintTo,
} from "@solana/spl-token";

import {
  PROGRAM_ID,
  contributeIx,
  contributorPda,
  fundraiserPda,
  initializeIx,
  refundIx,
  vaultPda,
} from "../src/client.js";

const RPC = process.env.RPC_URL ?? clusterApiUrl("devnet");
const DECIMALS = 6;

function loadKeypair(): Keypair {
  const path =
    process.env.SOLANA_KEYPAIR ?? `${homedir()}/.config/solana/id.json`;
  const secret = JSON.parse(readFileSync(path, "utf8"));
  return Keypair.fromSecretKey(Uint8Array.from(secret));
}

function explorer(sig: string): string {
  return `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
}

async function main() {
  const connection = new Connection(RPC, "confirmed");
  const payer = loadKeypair();
  console.log("Program:", PROGRAM_ID.toBase58());
  console.log("Payer:  ", payer.publicKey.toBase58());

  // Make sure the program is actually deployed.
  const programInfo = await connection.getAccountInfo(PROGRAM_ID);
  if (!programInfo || !programInfo.executable) {
    throw new Error(
      `Program ${PROGRAM_ID.toBase58()} is not deployed on devnet. ` +
        `Run ./devnet/deploy.sh and export FUNDRAISER_PROGRAM_ID.`,
    );
  }

  let balance = await connection.getBalance(payer.publicKey);
  console.log("Balance:", balance / LAMPORTS_PER_SOL, "SOL");
  if (balance < 0.05 * LAMPORTS_PER_SOL) {
    console.log("Low balance, requesting an airdrop...");
    try {
      const sig = await connection.requestAirdrop(
        payer.publicKey,
        LAMPORTS_PER_SOL,
      );
      await connection.confirmTransaction(sig, "confirmed");
    } catch (e) {
      console.warn("Airdrop failed (devnet faucet is often rate-limited):", e);
    }
  }

  // --- real SPL mint + contributor ATA ------------------------------------
  console.log("\nCreating mint...");
  const mint = await createMint(
    connection,
    payer,
    payer.publicKey,
    null,
    DECIMALS,
  );
  console.log("Mint:", mint.toBase58());

  const contributorAta = await createAssociatedTokenAccountIdempotent(
    connection,
    payer,
    mint,
    payer.publicKey,
  );
  await mintTo(connection, payer, mint, contributorAta, payer, 10_000_000);
  console.log("Contributor ATA:", contributorAta.toBase58(), "(10 tokens)");

  // --- fresh maker each run -----------------------------------------------
  // The fundraiser PDA is derived from ["fundraiser", maker], so a given maker
  // can only have ONE open campaign at a time (same as the Anchor original).
  // Using a fresh, payer-funded maker keeps this demo re-runnable.
  const maker = Keypair.generate();
  console.log("Maker:  ", maker.publicKey.toBase58(), "(fresh, funded by payer)");
  await sendAndConfirmTransaction(
    connection,
    new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: payer.publicKey,
        toPubkey: maker.publicKey,
        lamports: 0.02 * LAMPORTS_PER_SOL,
      }),
    ),
    [payer],
    { commitment: "confirmed" },
  );

  // The payer acts as the contributor (it holds the minted tokens).
  const contributor = payer;
  const [fundraiser, fundraiserBump] = fundraiserPda(maker.publicKey);
  const [vault, vaultBump] = vaultPda(fundraiser);
  const [cAccount, contributorBump] = contributorPda(
    fundraiser,
    contributor.publicKey,
  );

  const send = async (
    label: string,
    tx: Transaction,
    signers: Keypair[] = [payer],
  ) => {
    const sig = await sendAndConfirmTransaction(connection, tx, signers, {
      skipPreflight: false,
      commitment: "confirmed",
    });
    console.log(`  ✓ ${label}: ${explorer(sig)}`);
  };

  // --- initialize ---------------------------------------------------------
  console.log("\ninitialize (target 30 tokens, duration 0)...");
  await send(
    "initialize",
    new Transaction().add(
      initializeIx({
        maker: maker.publicKey,
        mint,
        fundraiser,
        vault,
        amount: 30_000_000n,
        duration: 0,
        fundraiserBump,
        vaultBump,
      }),
    ),
    [payer, maker],
  );

  // --- contribute ---------------------------------------------------------
  console.log("contribute (1 token)...");
  await send(
    "contribute",
    new Transaction().add(
      contributeIx({
        contributor: payer.publicKey,
        fundraiser,
        contributorAccount: cAccount,
        contributorAta,
        vault,
        amount: 1_000_000n,
        contributorBump,
      }),
    ),
  );
  console.log(
    "  vault balance:",
    (await getAccount(connection, vault)).amount.toString(),
  );

  // --- refund (campaign expired, target not met) --------------------------
  console.log("refund...");
  await send(
    "refund",
    new Transaction().add(
      refundIx({
        contributor: payer.publicKey,
        maker: maker.publicKey,
        fundraiser,
        contributorAccount: cAccount,
        contributorAta,
        vault,
        contributorBump,
      }),
    ),
  );
  console.log(
    "  contributor ATA balance after refund:",
    (await getAccount(connection, contributorAta)).amount.toString(),
  );

  console.log("\nDevnet flow completed successfully.");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
