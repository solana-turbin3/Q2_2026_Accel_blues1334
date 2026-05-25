const anchor = require("@coral-xyz/anchor");
const { Connection, PublicKey, Keypair } = require("@solana/web3.js");
const { init: initTuktuk, getTaskQueueForName } = require("@helium/tuktuk-sdk");
const { init: initCron, getCronJobForName } = require("@helium/cron-sdk");
const fs = require("fs");
const path = require("path");

(async () => {
  const connection = new Connection(process.env.RPC_URL || "https://api.devnet.solana.com", "confirmed");
  const keypath = path.resolve(process.env.HOME, ".config/solana/id.json");
  const wallet = new anchor.Wallet(
    Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(keypath, "utf-8"))))
  );
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);

  const tuktuk = await initTuktuk(provider);
  const cron = await initCron(provider);

  const suffix = wallet.publicKey.toBase58().slice(0, 6).toLowerCase();
  const queueName = `gpt-tuktuk-${suffix}`;
  const cronName = `ask-gpt-${suffix}`;

  console.log("Wallet:    ", wallet.publicKey.toBase58());
  console.log("Queue name:", queueName);
  console.log("Cron name: ", cronName);

  const queuePk = await getTaskQueueForName(tuktuk, queueName);
  console.log("\nTask queue:", queuePk ? queuePk.toBase58() : "NOT FOUND");
  if (queuePk) {
    const q = await tuktuk.account.taskQueueV0.fetchNullable(queuePk);
    if (q) {
      console.log("  update_authority:", q.updateAuthority.toBase58());
      console.log("  min_crank_reward:", q.minCrankReward.toString(), "lamports");
      console.log("  capacity:", q.capacity);
    }
  }

  const cronPk = await getCronJobForName(cron, cronName);
  console.log("\nCron job:", cronPk ? cronPk.toBase58() : "NOT FOUND");
  if (cronPk) {
    const c = await cron.account.cronJobV0.fetchNullable(cronPk);
    if (c) {
      console.log("  schedule:        ", c.schedule);
      console.log("  authority:       ", c.authority?.toBase58?.());
      console.log("  task_queue:      ", c.taskQueue?.toBase58?.());
      console.log("  next_schedule_ts:", c.nextScheduleTs?.toString?.());
      console.log("  removed_from_queue:", c.removedFromQueue);
    }
    const balance = await connection.getBalance(cronPk);
    console.log("  balance:         ", balance / 1e9, "SOL");

    console.log("\nRecent signatures on cron job:");
    const sigs = await connection.getSignaturesForAddress(cronPk, { limit: 10 });
    for (const s of sigs) {
      const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
      console.log(" ", t, s.err ? "FAILED" : "OK", s.signature);
    }
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
