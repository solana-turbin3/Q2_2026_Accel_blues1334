const { Connection, PublicKey } = require("@solana/web3.js");
const RPC = "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";
const c = new Connection(RPC, "confirmed");

(async () => {
  const PROG = new PublicKey("9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt");
  const cronJob = new PublicKey("HajzZZNnqgr95g4GovKuG7V7FdNAvtqK2FxZBAhtVu4Q");
  const gptConfig = new PublicKey("78CeW1kDLMv1e2cA1AgXuAD5sxiQRpnVABmig9n53sM7");
  const interaction = new PublicKey("4KEU5i6EsyNG5b7MmoNhiquLMFUJbi1asB7mGcYUcXNF");
  const payerPda = new PublicKey("7a3BXHaJijeFt7RyzTNzi7qNWzuqCSuRTUqX7XP1NDtH");

  console.log("=== Recent signatures on YOUR program ===");
  const progSigs = await c.getSignaturesForAddress(PROG, { limit: 8 });
  for (const s of progSigs) {
    const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
    console.log(" ", t, s.err ? "FAILED" : "OK", s.signature);
  }

  console.log("\n=== Recent signatures on YOUR cron job ===");
  const cronSigs = await c.getSignaturesForAddress(cronJob, { limit: 8 });
  for (const s of cronSigs) {
    const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
    console.log(" ", t, s.err ? "FAILED" : "OK", s.signature);
  }

  console.log("\n=== Interaction account (oracle waiting room) ===");
  const ixInfo = await c.getAccountInfo(interaction);
  if (ixInfo) {
    const isProcessed = ixInfo.data[ixInfo.data.length - 1] === 1;
    console.log("  is_processed:", isProcessed);
  } else {
    console.log("  NOT FOUND (was consumed/closed)");
  }

  console.log("\n=== GptConfig.latest_response ===");
  const cfgInfo = await c.getAccountInfo(gptConfig);
  if (cfgInfo) {
    let off = 8 + 32 + 32;
    const promptLen = cfgInfo.data.readUInt32LE(off); off += 4 + promptLen;
    const responseLen = cfgInfo.data.readUInt32LE(off); off += 4;
    const response = cfgInfo.data.subarray(off, off + responseLen).toString("utf-8");
    console.log("  response:", response || "(empty)");
  }

  console.log("\n=== Payer PDA balance ===");
  const bal = await c.getBalance(payerPda);
  console.log(" ", bal / 1e9, "SOL");
})().catch(e => { console.error(e); process.exit(1); });
