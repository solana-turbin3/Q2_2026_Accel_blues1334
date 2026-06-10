// End-to-end devnet run: create_collection -> initialize -> mint_asset -> stake
// -> claim_rewards -> unstake. Prints the devnet tx signature of each operation.
//
//   node scripts/devnet_e2e.js
//
// Rewards/freeze are measured in whole DAYS, so on a real-time devnet run within
// seconds: freeze_period is set to 0 (so unstake works immediately) and
// claim_rewards succeeds but mints 0 (less than one staked day elapsed). The
// reward math itself is proven in the LiteSVM tests via clock warping.

const fs = require("fs");
const {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} = require("@solana/web3.js");

const RPC = "https://api.devnet.solana.com";
const WALLET =
  "/home/blues/PRJ-solana/turbin3/mbeta/solana-starter/ts/turbin3-wallet.json";

const PROGRAM_ID = new PublicKey("5ENTKY4nGmnwAvcEM3xnE8UeAaB4K3UFcgw995cfhwbS");
const MPL_CORE = new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
const TOKEN = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROG = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const SYS = new PublicKey("11111111111111111111111111111111");

const DISC = {
  initialize: [175, 175, 109, 31, 13, 152, 155, 237],
  create_collection: [156, 251, 92, 54, 233, 2, 16, 82],
  mint_asset: [84, 175, 211, 156, 56, 250, 104, 118],
  stake: [206, 176, 202, 18, 200, 209, 179, 108],
  claim_rewards: [4, 144, 132, 71, 116, 23, 151, 80],
  unstake: [90, 95, 107, 42, 205, 124, 50, 225],
};

const m = (pubkey, isSigner, isWritable) => ({ pubkey, isSigner, isWritable });
const borshStr = (s) => {
  const b = Buffer.from(s, "utf8");
  const len = Buffer.alloc(4);
  len.writeUInt32LE(b.length);
  return Buffer.concat([len, b]);
};
const u16 = (n) => {
  const b = Buffer.alloc(2);
  b.writeUInt16LE(n);
  return b;
};
const pda = (seeds) => PublicKey.findProgramAddressSync(seeds, PROGRAM_ID)[0];

async function main() {
  const conn = new Connection(RPC, "confirmed");
  const payer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(WALLET)))
  );

  const collection = Keypair.generate();
  const asset = Keypair.generate();

  const updateAuthority = pda([
    Buffer.from("update_authority"),
    collection.publicKey.toBuffer(),
  ]);
  const config = pda([Buffer.from("config"), collection.publicKey.toBuffer()]);
  const rewardsMint = pda([Buffer.from("rewards_mint"), config.toBuffer()]);
  const userAta = PublicKey.findProgramAddressSync(
    [payer.publicKey.toBuffer(), TOKEN.toBuffer(), rewardsMint.toBuffer()],
    ATA_PROG
  )[0];

  console.log("payer       ", payer.publicKey.toBase58());
  console.log("collection  ", collection.publicKey.toBase58());
  console.log("asset       ", asset.publicKey.toBase58());
  console.log("rewardsMint ", rewardsMint.toBase58());
  console.log("");

  const results = [];
  const send = async (label, ix, signers) => {
    const tx = new Transaction().add(ix);
    const sig = await sendAndConfirmTransaction(conn, tx, [payer, ...signers], {
      commitment: "confirmed",
    });
    console.log(`✓ ${label.padEnd(16)} ${sig}`);
    results.push({ label, sig });
  };

  // 1. create_collection(name, uri)
  await send(
    "create_collection",
    new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        m(payer.publicKey, true, true),
        m(collection.publicKey, true, true),
        m(updateAuthority, false, false),
        m(MPL_CORE, false, false),
        m(SYS, false, false),
      ],
      data: Buffer.concat([
        Buffer.from(DISC.create_collection),
        borshStr("Core Stakers"),
        borshStr("https://example.com/collection.json"),
      ]),
    }),
    [collection]
  );

  // 2. initialize(rewards_bps=500, freeze_period=0)
  await send(
    "initialize",
    new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        m(payer.publicKey, true, true),
        m(collection.publicKey, false, false),
        m(updateAuthority, false, false),
        m(config, false, true),
        m(rewardsMint, false, true),
        m(TOKEN, false, false),
        m(SYS, false, false),
      ],
      data: Buffer.concat([Buffer.from(DISC.initialize), u16(500), u16(0)]),
    }),
    []
  );

  // 3. mint_asset(name, uri)
  await send(
    "mint_asset",
    new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        m(payer.publicKey, true, true),
        m(asset.publicKey, true, true),
        m(collection.publicKey, false, true),
        m(updateAuthority, false, false),
        m(MPL_CORE, false, false),
        m(SYS, false, false),
      ],
      data: Buffer.concat([
        Buffer.from(DISC.mint_asset),
        borshStr("Staker #1"),
        borshStr("https://example.com/asset.json"),
      ]),
    }),
    [asset]
  );

  // 4. stake
  await send(
    "stake",
    new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        m(payer.publicKey, true, true),
        m(asset.publicKey, false, true),
        m(collection.publicKey, false, true),
        m(config, false, false),
        m(updateAuthority, false, false),
        m(MPL_CORE, false, false),
        m(SYS, false, false),
      ],
      data: Buffer.from(DISC.stake),
    }),
    []
  );

  // 5. claim_rewards (mints 0 here: <1 staked day in real time)
  await send(
    "claim_rewards",
    new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        m(payer.publicKey, true, true),
        m(asset.publicKey, false, true),
        m(collection.publicKey, false, true),
        m(config, false, false),
        m(updateAuthority, false, false),
        m(rewardsMint, false, true),
        m(userAta, false, true),
        m(MPL_CORE, false, false),
        m(TOKEN, false, false),
        m(ATA_PROG, false, false),
        m(SYS, false, false),
      ],
      data: Buffer.from(DISC.claim_rewards),
    }),
    []
  );

  // 6. unstake (freeze_period=0 -> allowed immediately)
  await send(
    "unstake",
    new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        m(payer.publicKey, true, true),
        m(asset.publicKey, false, true),
        m(collection.publicKey, false, true),
        m(config, false, false),
        m(updateAuthority, false, false),
        m(rewardsMint, false, true),
        m(userAta, false, true),
        m(MPL_CORE, false, false),
        m(TOKEN, false, false),
        m(ATA_PROG, false, false),
        m(SYS, false, false),
      ],
      data: Buffer.from(DISC.unstake),
    }),
    []
  );

  console.log("\n--- markdown ---");
  console.log("| Operation | Devnet tx |");
  console.log("|---|---|");
  for (const r of results) {
    console.log(
      `| \`${r.label}\` | [\`${r.sig.slice(0, 16)}…\`](https://explorer.solana.com/tx/${r.sig}?cluster=devnet) |`
    );
  }
  console.log("\ncollection:", collection.publicKey.toBase58());
  console.log("asset:", asset.publicKey.toBase58());
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
