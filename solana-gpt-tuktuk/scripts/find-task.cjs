const anchor = require("@coral-xyz/anchor");
const { Connection, PublicKey } = require("@solana/web3.js");
const { init: initTuktuk } = require("@helium/tuktuk-sdk");

(async () => {
  const RPC = "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";
  const connection = new Connection(RPC, "confirmed");
  const provider = new anchor.AnchorProvider(connection, anchor.Wallet.local(), { commitment: "confirmed" });
  const tuktuk = await initTuktuk(provider);
  const queuePk = new PublicKey("D1RSm8dzCT1qrqLDKVsRLJTdakSRC89JQDxVLofm41fK");
  const TUKTUK_PROG = new PublicKey("tuktukUrfhXT6ZT77QTU8RQtvgL967uRuVagWF57zVA");

  // Derive task at index 0 (the active slot per bitmap)
  for (let idx = 0; idx < 4; idx++) {
    const idBuf = Buffer.alloc(2);
    idBuf.writeUInt16LE(idx);
    const [taskPk] = PublicKey.findProgramAddressSync([Buffer.from("task"), queuePk.toBuffer(), idBuf], TUKTUK_PROG);
    const info = await connection.getAccountInfo(taskPk);
    if (info) {
      console.log(`Task idx=${idx} PDA=${taskPk.toBase58()} size=${info.data.length}`);
      try {
        const t = await tuktuk.account.taskV0.fetchNullable(taskPk);
        if (t) {
          console.log("  trigger:", JSON.stringify(t.trigger));
          console.log("  crank_reward:", t.crankReward?.toString?.());
          console.log("  free_tasks:", t.freeTasks);
          console.log("  description:", t.description);
          console.log("  num_remaining_accounts:", t.transaction?.accounts?.length);
        }
      } catch (e) { console.log("  fetch err:", e.message); }

      console.log("\n  Recent signatures on this task:");
      const sigs = await connection.getSignaturesForAddress(taskPk, { limit: 5 });
      for (const s of sigs) {
        const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
        console.log("   ", t, s.err ? "FAILED:" + JSON.stringify(s.err) : "OK", s.signature);
      }
    }
  }

  console.log("\n=== Recent signatures on task queue ===");
  const qSigs = await connection.getSignaturesForAddress(queuePk, { limit: 8 });
  for (const s of qSigs) {
    const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
    console.log(" ", t, s.err ? "FAILED" : "OK", s.signature);
  }
})().catch(e => { console.error(e); process.exit(1); });
