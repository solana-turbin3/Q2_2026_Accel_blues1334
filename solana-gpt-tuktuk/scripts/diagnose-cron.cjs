const anchor = require("@coral-xyz/anchor");
const { Connection, PublicKey, Keypair } = require("@solana/web3.js");
const { init: initTuktuk } = require("@helium/tuktuk-sdk");
const { init: initCron, cronJobTransactionKey } = require("@helium/cron-sdk");
const fs = require("fs"), path = require("path");

(async () => {
  const RPC = "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";
  const connection = new Connection(RPC, "confirmed");
  const wallet = new anchor.Wallet(Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(path.resolve(process.env.HOME, ".config/solana/id.json"), "utf-8")))
  ));
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);
  const tuktuk = await initTuktuk(provider);
  const cron = await initCron(provider);

  const queuePk = new PublicKey("D1RSm8dzCT1qrqLDKVsRLJTdakSRC89JQDxVLofm41fK");
  const cronPk = new PublicKey("HajzZZNnqgr95g4GovKuG7V7FdNAvtqK2FxZBAhtVu4Q");

  console.log("=== Cron job full state ===");
  const c = await cron.account.cronJobV0.fetchNullable(cronPk);
  if (c) {
    console.log(JSON.stringify({
      schedule: c.schedule,
      authority: c.authority?.toBase58?.(),
      task_queue: c.taskQueue?.toBase58?.(),
      next_schedule_ts: c.nextScheduleTs?.toString?.(),
      current_exec_ts: c.currentExecTs?.toString?.(),
      num_transactions: c.numTransactions,
      next_transaction_id: c.nextTransactionId,
      removed_from_queue: c.removedFromQueue,
      free_tasks_per_transaction: c.freeTasksPerTransaction,
      num_tasks_per_queue_call: c.numTasksPerQueueCall,
      name: c.name,
    }, null, 2));
  }

  console.log("\n=== Cron transactions registered (slots 0..4) ===");
  for (let i = 0; i < 5; i++) {
    const [txPk] = cronJobTransactionKey(cronPk, i);
    const info = await connection.getAccountInfo(txPk);
    console.log(` slot ${i} (${txPk.toBase58()}):`, info ? `EXISTS (${info.data.length} bytes)` : "empty");
  }

  console.log("\n=== Task queue state ===");
  const q = await tuktuk.account.taskQueueV0.fetchNullable(queuePk);
  if (q) {
    console.log(JSON.stringify({
      capacity: q.capacity,
      min_crank_reward: q.minCrankReward.toString(),
      update_authority: q.updateAuthority.toBase58(),
      name: q.name,
      num_queue_authorities: q.numQueueAuthorities,
      stale_task_age: q.staleTaskAge?.toString?.(),
      task_bitmap_first_bytes: q.taskBitmap?.slice?.(0, 16),
    }, null, 2));
  }

  // Check the next_cron_transaction_id
  console.log("\n=== Cron approach: try to manually queue ===");
  const now = Math.floor(Date.now() / 1000);
  console.log("Current unix ts:", now);
  if (c?.nextScheduleTs) {
    console.log("next_schedule_ts:", c.nextScheduleTs.toString(), "(", c.nextScheduleTs.toNumber() - now, "seconds from now)");
  }
})().catch(e => { console.error(e); process.exit(1); });
