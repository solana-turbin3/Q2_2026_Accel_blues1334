import * as anchor from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  init as initTuktuk,
  createTaskQueue,
  getTaskQueueForName,
  taskQueueAuthorityKey,
  compileTransaction,
} from "@helium/tuktuk-sdk";
import {
  init as initCron,
  createCronJob,
  cronJobTransactionKey,
  getCronJobForName,
} from "@helium/cron-sdk";
import { createHash } from "crypto";
import * as fs from "fs";
import * as path from "path";
import bs58 from "bs58";
import "dotenv/config";

// Program IDs
const PROGRAM_ID = new PublicKey("9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt");
const ORACLE_PROGRAM_ID = new PublicKey("LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab");

// Derive PDAs
const [gptConfig] = PublicKey.findProgramAddressSync(
  [Buffer.from("gpt_config")],
  PROGRAM_ID
);
const [payerPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("payer")],
  PROGRAM_ID
);

function anchorDiscriminator(ixName: string): Buffer {
  return createHash("sha256").update(`global:${ixName}`).digest().subarray(0, 8);
}

async function loadAdmin(): Promise<Keypair> {
  const secretKey = process.env.ADMIN_SECRET_KEY;
  if (secretKey) {
    try {
      return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(secretKey)));
    } catch {
      return Keypair.fromSecretKey(bs58.decode(secretKey));
    }
  }
  const keypath = path.resolve(process.env.HOME!, ".config/solana/id.json");
  return Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(keypath, "utf-8")))
  );
}

async function sendInstructions(
  provider: anchor.AnchorProvider,
  ixs: TransactionInstruction[]
): Promise<string> {
  const connection = provider.connection;
  const payer = provider.wallet;
  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash("confirmed");
  const tx = new Transaction();
  tx.add(...ixs);
  tx.feePayer = payer.publicKey;
  tx.recentBlockhash = blockhash;
  const signed = await payer.signTransaction(tx);
  const sig = await connection.sendRawTransaction(signed.serialize(), { skipPreflight: true });
  await connection.confirmTransaction({ signature: sig, blockhash, lastValidBlockHeight }, "confirmed");
  return sig;
}

async function ensureTaskQueue(
  tuktukProgram: any,
  wallet: anchor.Wallet,
  name: string,
  provider: anchor.AnchorProvider
): Promise<PublicKey> {
  let taskQueue = await getTaskQueueForName(tuktukProgram, name);
  if (!taskQueue) {
    console.log(`Creating task queue "${name}"...`);
    const builder = await createTaskQueue(tuktukProgram, {
      name,
      // 50000 lamports per crank — covers base tx fee (~5000) + priority margin.
      // Without this, the crank turner rejects our tasks with "FeeTooHigh"
      // because it computes the cost of executing the tx and refuses to lose money.
      minCrankReward: new anchor.BN(50000),
      capacity: 100,
      lookupTables: [],
      staleTaskAge: 60 * 60 * 24, // 24h
    });
    // Anchor's sendAndConfirm via Helius is flaky — extract the ix and pubkeys, send via our helper
    const pubkeys = await (builder as any).pubkeys();
    const ix = await (builder as any).instruction();
    const sig = await sendInstructions(provider, [ix]);
    console.log("  Task queue create tx:", sig);
    taskQueue = pubkeys.taskQueue as PublicKey;
    console.log("  Task queue created:", taskQueue.toBase58());
  } else {
    console.log("Task queue (existing):", taskQueue.toBase58());
  }

  const queueAuthority = taskQueueAuthorityKey(taskQueue, wallet.publicKey)[0];
  const qaAccount = await tuktukProgram.account.taskQueueAuthorityV0.fetchNullable(
    queueAuthority
  );
  if (!qaAccount) {
    console.log("Adding wallet as queue authority...");
    await tuktukProgram.methods
      .addQueueAuthorityV0()
      .accounts({
        payer: wallet.publicKey,
        queueAuthority: wallet.publicKey,
        taskQueue,
      })
      .rpc();
  }

  return taskQueue;
}

async function main() {
  const rpcUrl = process.env.RPC_URL || "https://api.devnet.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  const adminKeypair = await loadAdmin();
  const wallet = new anchor.Wallet(adminKeypair);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Names must be unique per-creator on Tuktuk — derive a wallet-specific suffix
  // so we don't collide with task queues other bootcamp students registered.
  // -v2 because the original queue was created with minCrankReward=0 and the crank
  // turner refused our tasks with "FeeTooHigh". This version pays per crank.
  const walletSuffix = adminKeypair.publicKey.toBase58().slice(0, 6).toLowerCase();
  // Reusing -v2: queue+cron were created previously but never funded/configured.
  // Tuktuk now requires 1 SOL min_deposit for fresh queues, so we keep v2.
  const taskQueueName = process.env.TASK_QUEUE_NAME || `gpt-tuktuk-${walletSuffix}-v2`;
  const cronName = process.env.CRON_NAME || `ask-gpt-${walletSuffix}-v2`;
  // 6-field cron: sec min hour dom month dow (Tuktuk cron-sdk uses 6 fields)
  // Default: every 1 minute (good for testing)
  const schedule = process.env.CRON_SCHEDULE || "0 * * * * *";
  // 0.02 SOL = 20M lamports → 20M / 50000 reward = 400 cron executions
  const fundingLamports = Number(process.env.FUNDING_LAMPORTS || 0.02 * LAMPORTS_PER_SOL);

  const tuktukProgram = await initTuktuk(provider);
  const cronProgram = await initCron(provider);

  console.log("Wallet:        ", wallet.publicKey.toBase58());
  console.log("GptConfig PDA: ", gptConfig.toBase58());
  console.log("Payer PDA:     ", payerPda.toBase58());
  console.log("Schedule:      ", schedule);

  // 1. Ensure task queue exists
  const taskQueue = await ensureTaskQueue(tuktukProgram, wallet, taskQueueName, provider);

  // 2. Get or create cron job
  let cronJob = await getCronJobForName(cronProgram, cronName);
  if (!cronJob) {
    console.log(`Creating cron job "${cronName}"...`);
    const builder = await createCronJob(cronProgram, {
      tuktukProgram,
      taskQueue,
      args: {
        name: cronName,
        schedule,
        freeTasksPerTransaction: 1,
        numTasksPerQueueCall: 1,
      },
    });
    const pubkeys = await (builder as any).pubkeys();
    const ix = await (builder as any).instruction();
    const sig = await sendInstructions(provider, [ix]);
    console.log("  Cron job create tx:", sig);
    cronJob = pubkeys.cronJob as PublicKey;
    console.log("  Cron job created:", cronJob.toBase58());
  } else {
    console.log("Cron job (existing):", cronJob.toBase58());
  }

  // 2b. Fund cron job if balance is low. Idempotent — only tops up if needed.
  const cronBalance = await connection.getBalance(cronJob);
  const minBalance = 0.005 * LAMPORTS_PER_SOL;
  if (cronBalance < minBalance) {
    const toSend = fundingLamports;
    console.log(`  Cron job balance ${cronBalance / LAMPORTS_PER_SOL} SOL < ${minBalance / LAMPORTS_PER_SOL}, funding ${toSend / LAMPORTS_PER_SOL} SOL...`);
    const fundSig = await sendInstructions(provider, [
      SystemProgram.transfer({
        fromPubkey: adminKeypair.publicKey,
        toPubkey: cronJob,
        lamports: toSend,
      }),
    ]);
    console.log("  Fund tx:", fundSig);
  } else {
    console.log(`  Cron job already funded (${cronBalance / LAMPORTS_PER_SOL} SOL), skipping`);
  }

  // 3. Read GptConfig.context_account
  const gptConfigInfo = await connection.getAccountInfo(gptConfig);
  if (!gptConfigInfo) throw new Error("GptConfig not initialized. Run `yarn test` first.");
  const contextAccount = new PublicKey(gptConfigInfo.data.subarray(40, 72));
  console.log("Context account:", contextAccount.toBase58());

  // 4. Derive oracle Interaction PDA (deterministic from payer_pda + context)
  const [interaction] = PublicKey.findProgramAddressSync(
    [Buffer.from("interaction"), payerPda.toBuffer(), contextAccount.toBuffer()],
    ORACLE_PROGRAM_ID
  );
  console.log("Interaction PDA:", interaction.toBase58());

  // 5. Build the ask_gpt instruction Tuktuk will execute on each tick
  const askGptIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: gptConfig, isSigner: false, isWritable: false },
      { pubkey: payerPda, isSigner: false, isWritable: true },
      { pubkey: interaction, isSigner: false, isWritable: true },
      { pubkey: contextAccount, isSigner: false, isWritable: false },
      { pubkey: ORACLE_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.from(anchorDiscriminator("ask_gpt")),
  });

  // 6. Compile into a CompiledTransactionV0. No PDA seeds — ask_gpt does its
  // own invoke_signed internally for the payer PDA.
  const { transaction, remainingAccounts } = compileTransaction([askGptIx], []);

  // 7. Find a free cron transaction index
  let cronTxIndex = 0;
  for (let i = 0; i < 20; i++) {
    const [cronJobTx] = cronJobTransactionKey(cronJob, i);
    const info = await connection.getAccountInfo(cronJobTx, "confirmed");
    if (!info) {
      cronTxIndex = i;
      break;
    }
  }
  console.log("Adding cron transaction at index:", cronTxIndex);

  const addCronIx = await cronProgram.methods
    .addCronTransactionV0({
      index: cronTxIndex,
      transactionSource: {
        compiledV0: [transaction],
      },
    })
    .accounts({
      payer: adminKeypair.publicKey,
      cronJob,
      cronJobTransaction: cronJobTransactionKey(cronJob, cronTxIndex)[0],
    })
    .remainingAccounts(remainingAccounts)
    .instruction();

  const sig = await sendInstructions(provider, [addCronIx]);
  console.log("addCronTransactionV0 tx:", sig);

  console.log("\n=== Setup complete ===");
  console.log("Task queue:", taskQueue.toBase58());
  console.log("Cron job:  ", cronJob.toBase58());
  console.log("\nNow run `yarn crank` in another terminal to execute the schedule.");
}

main().catch((err) => {
  console.error("Error:", err);
  process.exit(1);
});
