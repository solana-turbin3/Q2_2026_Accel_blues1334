const anchor = require("@coral-xyz/anchor");
const { Connection, PublicKey, Keypair } = require("@solana/web3.js");
const { init: initTuktuk } = require("@helium/tuktuk-sdk");
const { init: initCron } = require("@helium/cron-sdk");
const fs = require("fs"), path = require("path");

(async () => {
  const RPC = "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";
  const conn = new Connection(RPC, "confirmed");
  const wallet = new anchor.Wallet(Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(path.resolve(process.env.HOME, ".config/solana/id.json"), "utf-8")))
  ));
  const provider = new anchor.AnchorProvider(conn, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);
  const tuktuk = await initTuktuk(provider);

  const PROG = new PublicKey("9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt");
  const cronV2 = new PublicKey("9WgdTdQ8i3ZuURt9epgViMgVs7Ezh1rJVjQjUECK32XC");
  const queueV2 = new PublicKey("BACUbkyYirqJHR5wd6SXu6PpXR9Y94k5B1Aat4uzusTG");
  const gptConfig = new PublicKey("78CeW1kDLMv1e2cA1AgXuAD5sxiQRpnVABmig9n53sM7");
  const interaction = new PublicKey("4KEU5i6EsyNG5b7MmoNhiquLMFUJbi1asB7mGcYUcXNF");

  console.log("=== YOUR cron job v2 — recent signatures ===");
  const cs = await conn.getSignaturesForAddress(cronV2, { limit: 10 });
  for (const s of cs) {
    const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
    console.log(" ", t, s.err ? "FAILED" : "OK", s.signature);
  }

  console.log("\n=== YOUR task queue v2 — recent signatures (cron tasks fired) ===");
  const qs = await conn.getSignaturesForAddress(queueV2, { limit: 10 });
  for (const s of qs) {
    const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
    console.log(" ", t, s.err ? "FAILED" : "OK", s.signature);
  }

  console.log("\n=== YOUR program — recent signatures (ask_gpt executions) ===");
  const ps = await conn.getSignaturesForAddress(PROG, { limit: 10 });
  for (const s of ps) {
    const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
    console.log(" ", t, s.err ? "FAILED" : "OK", s.signature);
  }

  console.log("\n=== Active tasks in queue v2 ===");
  const q = await tuktuk.account.taskQueueV0.fetchNullable(queueV2);
  if (q) {
    console.log(" capacity:", q.capacity, "min_crank_reward:", q.minCrankReward.toString());
    let active = 0;
    for (const b of q.taskBitmap) { for (let i = 0; i < 8; i++) if ((b >> i) & 1) active++; }
    console.log(" active tasks count:", active);
  }

  console.log("\n=== Cron job v2 state ===");
  const cron = await initCron(provider);
  const c = await cron.account.cronJobV0.fetchNullable(cronV2);
  if (c) {
    console.log(" current_exec_ts:", c.currentExecTs?.toString?.());
    console.log(" next_schedule_ts:", c.nextScheduleTs?.toString?.());
    console.log(" num_transactions:", c.numTransactions);
    console.log(" balance:", (await conn.getBalance(cronV2)) / 1e9, "SOL");
    const now = Math.floor(Date.now() / 1000);
    console.log(" now:", now);
  }

  console.log("\n=== GPT response ===");
  const cfg = await conn.getAccountInfo(gptConfig);
  if (cfg) {
    let off = 8 + 32 + 32;
    const pl = cfg.data.readUInt32LE(off); off += 4 + pl;
    const rl = cfg.data.readUInt32LE(off); off += 4;
    const resp = cfg.data.subarray(off, off + rl).toString("utf-8");
    console.log(" response:", resp || "(empty)");
  }

  const ix = await conn.getAccountInfo(interaction);
  console.log(" Interaction is_processed:", ix ? (ix.data[ix.data.length - 1] === 1) : "NOT_FOUND");
})().catch(e => { console.error(e); process.exit(1); });
