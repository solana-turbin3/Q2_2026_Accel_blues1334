const anchor = require("@coral-xyz/anchor");
const { Connection, PublicKey, Keypair } = require("@solana/web3.js");
const { init: initTuktuk, getTaskQueueForName } = require("@helium/tuktuk-sdk");
const { init: initCron, getCronJobForName } = require("@helium/cron-sdk");
const fs = require("fs"), path = require("path");

(async () => {
  const RPC = "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";
  const conn = new Connection(RPC, "confirmed");
  const wallet = new anchor.Wallet(Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(path.resolve(process.env.HOME, ".config/solana/id.json"), "utf-8")))));
  const provider = new anchor.AnchorProvider(conn, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);
  const tuktuk = await initTuktuk(provider);
  const cron = await initCron(provider);

  console.log("=== Tuktuk program errors ===");
  for (const e of (tuktuk.idl.errors || []).slice(0, 15)) {
    console.log(` ${e.code}: ${e.name} - ${e.msg || ''}`);
  }

  console.log("\n=== Check v3 state ===");
  const suffix = wallet.publicKey.toBase58().slice(0, 6).toLowerCase();
  const q3 = await getTaskQueueForName(tuktuk, `gpt-tuktuk-${suffix}-v3`);
  const c3 = await getCronJobForName(cron, `ask-gpt-${suffix}-v3`);
  console.log(" queue v3:", q3 ? q3.toBase58() : "NOT FOUND");
  console.log(" cron v3:", c3 ? c3.toBase58() : "NOT FOUND");

  console.log("\n=== TuktukConfig (global) ===");
  const [tuktukConfig] = PublicKey.findProgramAddressSync([Buffer.from("tuktuk_config")], tuktuk.programId);
  console.log(" TuktukConfig PDA:", tuktukConfig.toBase58());
  const cfgInfo = await conn.getAccountInfo(tuktukConfig);
  console.log(" exists:", cfgInfo ? "yes" : "NO");
  if (cfgInfo) {
    const cfg = await tuktuk.account.tuktukConfigV0.fetchNullable(tuktukConfig);
    if (cfg) {
      console.log(" min_deposit:", cfg.minDeposit?.toString?.());
      console.log(" next_task_queue_id:", cfg.nextTaskQueueId);
    }
  }
})().catch(e => { console.error(e); process.exit(1); });
