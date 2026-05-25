// Nukes old cron jobs + task queues created by this wallet.
// Best-effort: tries each step, logs failures, recovers SOL where possible.

const anchor = require("@coral-xyz/anchor");
const { Connection, PublicKey, Keypair } = require("@solana/web3.js");
const { init: initTuktuk, getTaskQueueForName, taskQueueAuthorityKey } = require("@helium/tuktuk-sdk");
const { init: initCron, getCronJobForName, cronJobTransactionKey } = require("@helium/cron-sdk");
const fs = require("fs"), path = require("path");

const RPC = process.env.RPC_URL || "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";

async function closeCronJob(cronProgram, name, wallet) {
  const cronPk = await getCronJobForName(cronProgram, name);
  if (!cronPk) { console.log(`  cron "${name}": NOT FOUND, skip`); return; }
  console.log(`  cron "${name}" @ ${cronPk.toBase58()}`);

  let c;
  try { c = await cronProgram.account.cronJobV0.fetchNullable(cronPk); }
  catch (e) { console.log("    fetch err:", e.message); return; }
  if (!c) { console.log("    fetch null"); return; }

  // Remove each cron transaction
  for (let i = 0; i < c.numTransactions; i++) {
    const [txPk] = cronJobTransactionKey(cronPk, i);
    try {
      const sig = await cronProgram.methods.removeCronTransactionV0({ index: i })
        .accounts({
          rentRefund: wallet.publicKey,
          authority: wallet.publicKey,
          cronJob: cronPk,
          cronJobTransaction: txPk,
        })
        .rpc({ skipPreflight: true });
      console.log(`    removed cronTransaction[${i}] sig=${sig.slice(0, 10)}...`);
    } catch (e) {
      console.log(`    remove cronTransaction[${i}] err:`, e.message?.slice(0, 100));
    }
  }

  // Close the cron job
  try {
    const sig = await cronProgram.methods.closeCronJobV0()
      .accounts({
        rentRefund: wallet.publicKey,
        authority: wallet.publicKey,
        cronJob: cronPk,
      })
      .rpc({ skipPreflight: true });
    console.log(`    CLOSED cron job, sig=${sig.slice(0, 10)}...`);
  } catch (e) {
    console.log("    closeCronJobV0 err:", e.message?.slice(0, 200));
  }
}

async function closeTaskQueue(tuktukProgram, name, wallet) {
  const queuePk = await getTaskQueueForName(tuktukProgram, name);
  if (!queuePk) { console.log(`  queue "${name}": NOT FOUND, skip`); return; }
  console.log(`  queue "${name}" @ ${queuePk.toBase58()}`);

  // Remove our queue authority
  const [qa] = taskQueueAuthorityKey(queuePk, wallet.publicKey);
  try {
    const sig = await tuktukProgram.methods.removeQueueAuthorityV0()
      .accounts({
        rentRefund: wallet.publicKey,
        updateAuthority: wallet.publicKey,
        queueAuthority: wallet.publicKey,
        taskQueue: queuePk,
        taskQueueAuthority: qa,
      })
      .rpc({ skipPreflight: true });
    console.log(`    removed queue authority, sig=${sig.slice(0, 10)}...`);
  } catch (e) {
    console.log("    removeQueueAuthority err:", e.message?.slice(0, 100));
  }

  // Close the task queue
  try {
    const sig = await tuktukProgram.methods.closeTaskQueueV0()
      .accounts({
        rentRefund: wallet.publicKey,
        updateAuthority: wallet.publicKey,
        taskQueue: queuePk,
      })
      .rpc({ skipPreflight: true });
    console.log(`    CLOSED task queue, sig=${sig.slice(0, 10)}...`);
  } catch (e) {
    console.log("    closeTaskQueueV0 err:", e.message?.slice(0, 200));
  }
}

(async () => {
  const conn = new Connection(RPC, "confirmed");
  const wallet = new anchor.Wallet(Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(path.resolve(process.env.HOME, ".config/solana/id.json"), "utf-8")))
  ));
  const provider = new anchor.AnchorProvider(conn, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);
  const tuktuk = await initTuktuk(provider);
  const cron = await initCron(provider);

  const suffix = wallet.publicKey.toBase58().slice(0, 6).toLowerCase();

  console.log("=== Balance before ===");
  console.log(" ", (await conn.getBalance(wallet.publicKey)) / 1e9, "SOL");

  console.log("\n=== Closing cron jobs ===");
  for (const variant of ["", "-v2"]) {
    await closeCronJob(cron, `ask-gpt-${suffix}${variant}`, wallet);
  }

  console.log("\n=== Closing task queues ===");
  for (const variant of ["", "-v2"]) {
    await closeTaskQueue(tuktuk, `gpt-tuktuk-${suffix}${variant}`, wallet);
  }

  console.log("\n=== Balance after ===");
  console.log(" ", (await conn.getBalance(wallet.publicKey)) / 1e9, "SOL");
  console.log("\nNow run `yarn cron` to create fresh -v3.");
})().catch(e => { console.error(e); process.exit(1); });
