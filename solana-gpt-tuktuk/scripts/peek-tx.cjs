const { Connection } = require("@solana/web3.js");
const RPC = "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";
const c = new Connection(RPC, "confirmed");
(async () => {
  const sig = "2Vz8t99Dwoz27gEAqVLSK1gvi1gs2hSnt3PQWx8EwCnD85cUBwwKhFQscqiA8z1MkmPfS41KsmQNhe26Q3VnrWJU";
  const tx = await c.getTransaction(sig, { maxSupportedTransactionVersion: 0 });
  if (!tx) { console.log("not found"); return; }
  console.log("Block time:", new Date(tx.blockTime * 1000).toISOString());
  console.log("Tx error:", tx.meta.err || "none");
  console.log("\nLogs:");
  for (const l of tx.meta.logMessages) console.log(" ", l);
})().catch(e => console.error(e));
