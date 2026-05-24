const { Connection, PublicKey } = require("@solana/web3.js");
const RPC = "https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695";
const c = new Connection(RPC, "confirmed");

(async () => {
  // Reference project
  const REF_PROG = new PublicKey("H8Tq9DAw82BcYzeeBpm3BLisK8sQn4Ntyj3AewhNTuvj");
  const ORACLE = new PublicKey("LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab");
  const [refGptConfig] = PublicKey.findProgramAddressSync([Buffer.from("gpt_config")], REF_PROG);
  const [refPayerPda] = PublicKey.findProgramAddressSync([Buffer.from("payer")], REF_PROG);

  console.log("=== REFERENCE PROJECT (solana-gpt-tuktuk original) ===");
  console.log("Program ID: ", REF_PROG.toBase58());
  console.log("GptConfig:  ", refGptConfig.toBase58());

  const cfg = await c.getAccountInfo(refGptConfig);
  if (!cfg) {
    console.log("\nGptConfig NOT FOUND — reference project never initialized");
    return;
  }

  console.log("\nGptConfig data:");
  let off = 8;
  const admin = new PublicKey(cfg.data.subarray(off, off + 32)); off += 32;
  const ctx = new PublicKey(cfg.data.subarray(off, off + 32)); off += 32;
  const pl = cfg.data.readUInt32LE(off); off += 4;
  const prompt = cfg.data.subarray(off, off + pl).toString("utf-8"); off += pl;
  const rl = cfg.data.readUInt32LE(off); off += 4;
  const response = cfg.data.subarray(off, off + rl).toString("utf-8");

  console.log("  admin:   ", admin.toBase58());
  console.log("  context: ", ctx.toBase58());
  console.log("  prompt:  ", JSON.stringify(prompt));
  console.log("  response:", response ? JSON.stringify(response) : "(empty - never got GPT callback)");
  console.log("  response length:", rl);

  // Check their Interaction
  const [interaction] = PublicKey.findProgramAddressSync(
    [Buffer.from("interaction"), refPayerPda.toBuffer(), ctx.toBuffer()],
    ORACLE
  );
  console.log("\nReference Interaction PDA:", interaction.toBase58());
  const ix = await c.getAccountInfo(interaction);
  if (ix) {
    console.log("  is_processed:", ix.data[ix.data.length - 1] === 1);
  } else {
    console.log("  NOT FOUND (consumed/closed)");
  }

  console.log("\n=== Reference program recent signatures ===");
  const sigs = await c.getSignaturesForAddress(REF_PROG, { limit: 8 });
  for (const s of sigs) {
    const t = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : "?";
    console.log(" ", t, s.err ? "FAILED" : "OK", s.signature);
  }
})().catch(e => { console.error(e); process.exit(1); });
