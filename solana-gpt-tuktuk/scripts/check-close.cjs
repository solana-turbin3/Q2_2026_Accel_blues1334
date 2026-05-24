const anchor = require("@coral-xyz/anchor");
const { Connection, Keypair } = require("@solana/web3.js");
const { init: initTuktuk } = require("@helium/tuktuk-sdk");
const { init: initCron } = require("@helium/cron-sdk");
const fs = require("fs"), path = require("path");
(async () => {
  const RPC = "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";
  const conn = new Connection(RPC, "confirmed");
  const wallet = new anchor.Wallet(Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(path.resolve(process.env.HOME, ".config/solana/id.json"), "utf-8")))));
  const provider = new anchor.AnchorProvider(conn, wallet, { commitment: "confirmed" });
  const tuktuk = await initTuktuk(provider);
  const cron = await initCron(provider);

  const findIx = (idl, name) => idl.instructions.find(i => i.name === name);
  console.log("=== cron-sdk closeCronJobV0 ===");
  console.log(JSON.stringify(findIx(cron.idl, "closeCronJobV0").accounts, null, 2));
  console.log("\n=== cron-sdk removeCronTransactionV0 ===");
  console.log(JSON.stringify(findIx(cron.idl, "removeCronTransactionV0"), null, 2));
  console.log("\n=== tuktuk-sdk closeTaskQueueV0 ===");
  console.log(JSON.stringify(findIx(tuktuk.idl, "closeTaskQueueV0").accounts, null, 2));
  console.log("\n=== tuktuk-sdk removeQueueAuthorityV0 ===");
  console.log(JSON.stringify(findIx(tuktuk.idl, "removeQueueAuthorityV0").accounts, null, 2));
})().catch(e => { console.error(e); process.exit(1); });
