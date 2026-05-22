import { AnchorProvider, BN, BorshCoder, Program, Wallet } from "@coral-xyz/anchor";
import {
  compileTransaction,
  createTaskQueue,
  customSignerKey,
  customSignerSeedsWithBumps,
  getTaskQueueForName,
  init as initTuktuk,
  taskQueueAuthorityKey,
} from "@helium/tuktuk-sdk";
import {
  createCronJob,
  cronJobTransactionKey,
  getCronJobForName,
  init as initCron,
} from "@helium/cron-sdk";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionMessage,
  TransactionInstruction,
  VersionedTransaction,
} from "@solana/web3.js";
import { createHash } from "crypto";
import * as fs from "fs";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";

function loadKeypair(path: string): Keypair {
  const rawData = fs.readFileSync(path);
  const secretKey = Uint8Array.from(JSON.parse(rawData.toString()));
  return Keypair.fromSecretKey(secretKey);
}

function anchorDiscriminator(ixName: string): Buffer {
  const preimage = `global:${ixName}`;
  return createHash("sha256").update(preimage).digest().subarray(0, 8);
}

async function sendInstructions(provider: AnchorProvider, ixs: any[]) {
  const tx = new Transaction();
  tx.add(...ixs);
  return provider.sendAndConfirm(tx, [], { skipPreflight: true });
}

async function sendInstructionsWithKeypair(
  connection: Connection,
  payer: Keypair,
  ixs: TransactionInstruction[],
  opts: { skipPreflight?: boolean } = {}
) {
  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash("confirmed");
  const messageV0 = new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: blockhash,
    instructions: ixs,
  }).compileToV0Message();

  const tx = new VersionedTransaction(messageV0);
  tx.sign([payer]);

  const sim = await connection.simulateTransaction(tx, {
    sigVerify: false,
    commitment: "confirmed",
  });
  if (sim.value.err) {
    console.error("Transaction simulation failed:", sim.value.err);
    if (sim.value.logs) {
      console.error("Simulation logs:\n" + sim.value.logs.join("\n"));
    }
    throw new Error("Transaction simulation failed (see logs above)");
  }

  const sig = await connection.sendRawTransaction(tx.serialize(), {
    skipPreflight: opts.skipPreflight ?? false,
  });

  await connection.confirmTransaction(
    {
      signature: sig,
      blockhash,
      lastValidBlockHeight,
    },
    "confirmed"
  );

  return sig;
}

function ensureIdlAddress(idl: any, programId: PublicKey) {
  if (!idl.metadata) idl.metadata = {};
  const addr = idl.metadata.address;
  try {
    if (typeof addr === "string") {
      // Validate
      // eslint-disable-next-line no-new
      new PublicKey(addr);
      return;
    }
  } catch (_e) {
    // fall through
  }
  idl.metadata.address = programId.toBase58();
}

function pickInstructionName(idl: any, candidates: string[]) {
  const names: string[] = Array.isArray(idl?.instructions)
    ? idl.instructions.map((i: any) => i?.name).filter(Boolean)
    : [];

  for (const c of candidates) {
    if (names.includes(c)) return c;
  }

  const fuzzy = names.find((n) => {
    const s = String(n).toLowerCase();
    return s.includes("request") && s.includes("random");
  });
  if (fuzzy) return fuzzy;

  throw new Error(
    `Could not find request randomness instruction in IDL. Tried: ${candidates.join(", ")}. Available: ${names.join(", ")}`
  );
}

async function initializeTaskQueue(program: any, name: string): Promise<PublicKey> {
  let taskQueue = await getTaskQueueForName(program, name);
  if (!taskQueue) {
    console.log("Creating task queue...", name);
    const { pubkeys: { taskQueue: taskQueuePubkey } } = await (await createTaskQueue(program, {
      name,
      minCrankReward: new BN(10000),
      capacity: 10,
      lookupTables: [],
      staleTaskAge: 60 * 60 * 48,
    })).rpcAndKeys();
    taskQueue = taskQueuePubkey;
  }

  const queueAuthority = taskQueueAuthorityKey(taskQueue, program.provider.wallet!.publicKey)[0];
  const queueAuthorityAccount = await program.account.taskQueueAuthorityV0.fetchNullable(queueAuthority);
  if (!queueAuthorityAccount) {
    console.log("Adding queue authority...", queueAuthority.toBase58());
    try {
      await program.methods
        .addQueueAuthorityV0()
        .accounts({
          payer: program.provider.wallet!.publicKey,
          queueAuthority: program.provider.wallet!.publicKey,
          taskQueue,
        })
        .rpc();
    } catch (e) {
      console.error("Failed to add queue authority");
      throw e;
    }
  }

  return taskQueue;
}

async function main() {
  const argv = await yargs(hideBin(process.argv))
    .options({
      rpcUrl: {
        type: "string",
        demandOption: true,
      },
      walletPath: {
        type: "string",
        demandOption: true,
      },
      queueName: {
        type: "string",
        demandOption: true,
      },
      cronName: {
        type: "string",
        demandOption: true,
      },
      erProgramId: {
        type: "string",
        demandOption: true,
      },
      oracleQueue: {
        type: "string",
        default: "Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh",
      },
      schedule: {
        type: "string",
        default: "0 */1 * * * *",
      },
      cronTxIndex: {
        type: "number",
        default: 0,
      },
      fundingAmount: {
        type: "number",
        default: 0.02 * LAMPORTS_PER_SOL,
      },
    })
    .help()
    .alias("help", "h").argv;

  const keypair = loadKeypair(argv.walletPath);
  const connection = new Connection(argv.rpcUrl, "confirmed");
  const wallet = new Wallet(keypair);
  const provider = new AnchorProvider(connection, wallet, { commitment: "confirmed" });

  const tuktukProgram = await initTuktuk(provider);
  const cronProgram = await initCron(provider);

  const taskQueue = await initializeTaskQueue(tuktukProgram, argv.queueName);

  let cronJob = await getCronJobForName(cronProgram, argv.cronName);
  if (!cronJob) {
    console.log("Creating cron job...", argv.cronName);
    const { pubkeys: { cronJob: cronJobPubkey } } = await (await createCronJob(cronProgram, {
      tuktukProgram,
      taskQueue,
      args: {
        name: argv.cronName,
        schedule: argv.schedule,
        freeTasksPerTransaction: 1,
        numTasksPerQueueCall: 1,
      },
    })).rpcAndKeys({ skipPreflight: true });
    cronJob = cronJobPubkey;

    await sendInstructions(provider, [
      SystemProgram.transfer({
        fromPubkey: keypair.publicKey,
        toPubkey: cronJob,
        lamports: argv.fundingAmount,
      }),
    ]);
  }

  const erProgramId = new PublicKey(argv.erProgramId);
  const oracleQueue = new PublicKey(argv.oracleQueue);

  let idl = await Program.fetchIdl(erProgramId, provider);
  if (!idl) {
    const localIdlPath = `${process.cwd()}/target/idl/er_state_account.json`;
    try {
      const raw = fs.readFileSync(localIdlPath, "utf-8");
      idl = JSON.parse(raw);
      console.log("Using local IDL:", localIdlPath);
    } catch (e) {
      throw new Error(
        `Failed to fetch IDL for program ${erProgramId.toBase58()} from ${argv.rpcUrl} and failed to read local IDL at ${localIdlPath}. ` +
          `You can fix this by running an Anchor deploy/idl init on devnet, or by ensuring the local IDL file exists.`
      );
    }
  }

  ensureIdlAddress(idl, erProgramId);

  const coder = new BorshCoder(idl as any);

  // Use a TukTuk custom signer PDA as the "payer" for program-level signing.
  // The crank turner can sign for this PDA using seeds, so the instruction's
  // Signer constraint can be satisfied without having the user's private key.
  const pdaSeeds = [Buffer.from("vrf")];
  const [taskPayer] = customSignerKey(taskQueue, pdaSeeds);

  // Ensure the PDA has some SOL (rent/fees for any downstream CPI that might require it)
  // This only needs to succeed once.
  try {
    await sendInstructions(provider, [
      SystemProgram.transfer({
        fromPubkey: keypair.publicKey,
        toPubkey: taskPayer,
        lamports: 0.01 * LAMPORTS_PER_SOL,
      }),
    ]);
  } catch (_e) {
    // Ignore funding errors (might already be funded or rate-limited). The crank can still try.
  }

  const [userAccount] = PublicKey.findProgramAddressSync(
    [Buffer.from("user"), taskPayer.toBuffer()],
    erProgramId
  );

  // If needed, initialize the user account for the PDA payer.
  // This uses a special instruction that does NOT require the PDA to sign.
  const existingUserAccount = await connection.getAccountInfo(userAccount, "confirmed");
  if (existingUserAccount) {
    if (!existingUserAccount.owner.equals(erProgramId)) {
      throw new Error(
        `user_account PDA ${userAccount.toBase58()} already exists but is owned by ${existingUserAccount.owner.toBase58()} (expected ${erProgramId.toBase58()}).`
      );
    }
  } else {
    console.log("Initializing PDA user_account...", userAccount.toBase58());
    const initPdaData = Buffer.from(anchorDiscriminator("initialize_pda"));
    const initPdaIx = new TransactionInstruction({
      programId: erProgramId,
      keys: [
        { pubkey: keypair.publicKey, isSigner: true, isWritable: true },
        { pubkey: taskPayer, isSigner: false, isWritable: false },
        { pubkey: userAccount, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data: initPdaData,
    });

    await sendInstructionsWithKeypair(connection, keypair, [initPdaIx]);

    const afterInit = await connection.getAccountInfo(userAccount, "confirmed");
    if (!afterInit) {
      throw new Error(`Failed to initialize user_account PDA ${userAccount.toBase58()}`);
    }
  }

  const requestIxName = pickInstructionName(idl, [
    "requestRandomness",
    "request_randomness",
    "requestRandomnessEr",
    "request_randomness_er",
  ]);

  const requestData = coder.instruction.encode(requestIxName, {
    seed: 7,
  });

  // The #[vrf] macro adds: program_identity, vrf_program, slot_hashes, system_program
  const VRF_PROGRAM_ID = new PublicKey("Vrf1RNUjXmQGjmQrQLvJHs9SNkvDJEsRVFPkfSQUwGz");
  const SLOT_HASHES = new PublicKey("SysvarS1otHashes111111111111111111111111111");
  const [programIdentity] = PublicKey.findProgramAddressSync(
    [Buffer.from("identity")],
    erProgramId
  );

  const requestIx = new TransactionInstruction({
    programId: erProgramId,
    keys: [
      { pubkey: taskPayer, isSigner: true, isWritable: true },
      { pubkey: userAccount, isSigner: false, isWritable: true },
      { pubkey: oracleQueue, isSigner: false, isWritable: true },
      { pubkey: programIdentity, isSigner: false, isWritable: false },
      { pubkey: VRF_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SLOT_HASHES, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: requestData,
  });

  const { transaction, remainingAccounts } = compileTransaction(
    [requestIx],
    customSignerSeedsWithBumps([pdaSeeds], taskQueue)
  );

  console.log("Adding cron transaction...");
  const cronTxIndexStart = Number(argv.cronTxIndex ?? 0);
  let cronTxIndex = cronTxIndexStart;
  for (let i = cronTxIndexStart; i < cronTxIndexStart + 20; i++) {
    const [cronJobTx] = cronJobTransactionKey(cronJob, i);
    const info = await connection.getAccountInfo(cronJobTx, "confirmed");
    if (!info) {
      cronTxIndex = i;
      break;
    }
  }

  if (cronTxIndex !== cronTxIndexStart) {
    console.log(
      `cron_job_transaction index ${cronTxIndexStart} already exists; using index ${cronTxIndex} instead.`
    );
  }
  const addCronIx = await cronProgram.methods
    .addCronTransactionV0({
      index: cronTxIndex,
      transactionSource: {
        compiledV0: [transaction],
      },
    })
    .accounts({
      payer: keypair.publicKey,
      cronJob,
      cronJobTransaction: cronJobTransactionKey(cronJob, cronTxIndex)[0],
    })
    .remainingAccounts(remainingAccounts)
    .instruction();

  await sendInstructionsWithKeypair(connection, keypair, [addCronIx], { skipPreflight: true });

  console.log("Cron job address:", cronJob.toBase58());
  console.log("Task queue address:", taskQueue.toBase58());
  console.log("User account PDA:", userAccount.toBase58());
  console.log("Schedule:", argv.schedule);
  console.log("\nTo run the crank turner (devnet):");
  console.log("tuktuk-crank-turner -c ./tuktuk/crank-devnet.toml");
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });
