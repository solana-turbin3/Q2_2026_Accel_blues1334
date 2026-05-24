import * as anchor from "@coral-xyz/anchor";
import { PublicKey, Keypair, Connection, SystemProgram } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";

// Program IDs
const PROGRAM_ID = new PublicKey("9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt");
const ORACLE_PROGRAM_ID = new PublicKey("LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab");

// Load wallet
const keypath = path.resolve(process.env.HOME!, ".config/solana/id.json");
const secretKey = Uint8Array.from(JSON.parse(fs.readFileSync(keypath, "utf-8")));
const wallet = Keypair.fromSecretKey(secretKey);

const connection = new Connection(
  process.env.RPC_URL || "https://api.devnet.solana.com",
  "confirmed"
);
const provider = new anchor.AnchorProvider(
  connection,
  new anchor.Wallet(wallet),
  { commitment: "confirmed" }
);
anchor.setProvider(provider);

// Derive PDAs
const [gptConfig] = PublicKey.findProgramAddressSync(
  [Buffer.from("gpt_config")],
  PROGRAM_ID
);
const [payerPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("payer")],
  PROGRAM_ID
);

// Oracle PDAs
const [oracleCounter] = PublicKey.findProgramAddressSync(
  [Buffer.from("counter")],
  ORACLE_PROGRAM_ID
);
const [oracleIdentity] = PublicKey.findProgramAddressSync(
  [Buffer.from("identity")],
  ORACLE_PROGRAM_ID
);

function disc(name: string): Buffer {
  return crypto.createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
}

async function step1_createOracleContext(): Promise<PublicKey> {
  console.log("\n--- Step 1: Create Oracle Context Account ---");

  const counterInfo = await connection.getAccountInfo(oracleCounter);
  if (!counterInfo) throw new Error("Oracle counter not found on devnet");

  const count = counterInfo.data.readUInt32LE(8);
  console.log("  Oracle counter:", count);

  const countBuf = Buffer.alloc(4);
  countBuf.writeUInt32LE(count);
  const [contextAccount] = PublicKey.findProgramAddressSync(
    [Buffer.from("test-context"), countBuf],
    ORACLE_PROGRAM_ID
  );
  console.log("  Context account PDA:", contextAccount.toBase58());

  const existing = await connection.getAccountInfo(contextAccount);
  if (existing) {
    console.log("  Context account already exists, skipping creation");
    return contextAccount;
  }

  const systemPrompt = "You are a helpful on-chain oracle. Give very short answers (under 100 chars).";
  const ixDisc = disc("create_llm_context");
  const textBytes = Buffer.from(systemPrompt, "utf-8");
  const textLen = Buffer.alloc(4);
  textLen.writeUInt32LE(textBytes.length);
  const data = Buffer.concat([ixDisc, textLen, textBytes]);

  const ix = new anchor.web3.TransactionInstruction({
    programId: ORACLE_PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: oracleCounter, isSigner: false, isWritable: true },
      { pubkey: contextAccount, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new anchor.web3.Transaction().add(ix);
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.sign(wallet);
  const sig = await connection.sendRawTransaction(tx.serialize(), { skipPreflight: true });
  await connection.confirmTransaction(sig, "confirmed");
  console.log("  Created context account, tx:", sig);
  return contextAccount;
}

async function step2_initialize(contextAccount: PublicKey) {
  console.log("\n--- Step 2: Initialize GptConfig ---");

  const existing = await connection.getAccountInfo(gptConfig);
  if (existing) {
    console.log("  GptConfig already initialized, skipping");
    return;
  }

  const prompt = "What is the current sentiment of Solana ecosystem in one sentence?";
  const ixDisc = disc("initialize");
  const promptBytes = Buffer.from(prompt, "utf-8");
  const promptLen = Buffer.alloc(4);
  promptLen.writeUInt32LE(promptBytes.length);
  const data = Buffer.concat([ixDisc, promptLen, promptBytes]);

  const ix = new anchor.web3.TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: gptConfig, isSigner: false, isWritable: true },
      { pubkey: contextAccount, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new anchor.web3.Transaction().add(ix);
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.sign(wallet);
  const sig = await connection.sendRawTransaction(tx.serialize(), { skipPreflight: true });
  await connection.confirmTransaction(sig, "confirmed");
  console.log("  Initialized GptConfig, tx:", sig);
}

async function step3_fundPayerPda() {
  console.log("\n--- Step 3: Fund Payer PDA (system-owned, for oracle rent) ---");
  console.log("  Payer PDA:", payerPda.toBase58());

  const balance = await connection.getBalance(payerPda);
  console.log("  Payer PDA balance:", balance / 1e9, "SOL");

  if (balance > 0.01 * 1e9) {
    console.log("  Already funded, skipping");
    return;
  }

  const tx = new anchor.web3.Transaction().add(
    SystemProgram.transfer({
      fromPubkey: wallet.publicKey,
      toPubkey: payerPda,
      lamports: 0.05 * 1e9,
    })
  );
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.sign(wallet);
  const sig = await connection.sendRawTransaction(tx.serialize());
  await connection.confirmTransaction(sig, "confirmed");
  console.log("  Funded Payer PDA with 0.05 SOL, tx:", sig);
}

async function step4_askGpt() {
  console.log("\n--- Step 4: Call ask_gpt (CPI to oracle) ---");

  // Read context_account from GptConfig
  const configInfo = await connection.getAccountInfo(gptConfig);
  if (!configInfo) throw new Error("GptConfig not found");
  const contextAccount = new PublicKey(configInfo.data.subarray(40, 72));
  console.log("  Context account (from config):", contextAccount.toBase58());

  // Derive oracle interaction PDA using payer PDA
  const [interaction] = PublicKey.findProgramAddressSync(
    [Buffer.from("interaction"), payerPda.toBuffer(), contextAccount.toBuffer()],
    ORACLE_PROGRAM_ID
  );
  console.log("  Interaction PDA:", interaction.toBase58());

  const ixDisc = disc("ask_gpt");
  const data = Buffer.from(ixDisc);

  const ix = new anchor.web3.TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: gptConfig, isSigner: false, isWritable: false },
      { pubkey: payerPda, isSigner: false, isWritable: true },
      { pubkey: interaction, isSigner: false, isWritable: true },
      { pubkey: contextAccount, isSigner: false, isWritable: false },
      { pubkey: ORACLE_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new anchor.web3.Transaction().add(ix);
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.sign(wallet);
  const sig = await connection.sendRawTransaction(tx.serialize(), { skipPreflight: true });
  await connection.confirmTransaction(sig, "confirmed");
  console.log("  ask_gpt sent, tx:", sig);
  console.log("  Oracle will process off-chain and call receive_response...");
}

async function step5_checkResponse() {
  console.log("\n--- Step 5: Check GptConfig for response ---");

  const info = await connection.getAccountInfo(gptConfig);
  if (!info) {
    console.log("  GptConfig not found!");
    return;
  }

  // Parse: 8 disc + 32 admin + 32 context_account + 4+N prompt + 4+N latest_response + 1 bump
  let offset = 8;
  const admin = new PublicKey(info.data.subarray(offset, offset + 32));
  offset += 32;
  const ctxAcct = new PublicKey(info.data.subarray(offset, offset + 32));
  offset += 32;

  const promptLen = info.data.readUInt32LE(offset);
  offset += 4;
  const prompt = info.data.subarray(offset, offset + promptLen).toString("utf-8");
  offset += promptLen;

  const responseLen = info.data.readUInt32LE(offset);
  offset += 4;
  const response = info.data.subarray(offset, offset + responseLen).toString("utf-8");
  offset += responseLen;

  const bump = info.data[offset];

  console.log("  Admin:", admin.toBase58());
  console.log("  Context:", ctxAcct.toBase58());
  console.log("  Prompt:", prompt);
  console.log("  Latest Response:", response || "(empty - oracle hasn't called back yet)");
  console.log("  Bump:", bump);
}

async function main() {
  console.log("=== Solana GPT Tuktuk Test ===");
  console.log("Program:", PROGRAM_ID.toBase58());
  console.log("Oracle:", ORACLE_PROGRAM_ID.toBase58());
  console.log("Wallet:", wallet.publicKey.toBase58());
  console.log("GptConfig PDA:", gptConfig.toBase58());
  console.log("Payer PDA:", payerPda.toBase58());
  console.log("Oracle Identity PDA:", oracleIdentity.toBase58());

  const contextAccount = await step1_createOracleContext();
  await step2_initialize(contextAccount);
  await step3_fundPayerPda();
  await step4_askGpt();

  // Wait then check for response
  console.log("\n  Waiting 15s for oracle processing...");
  await new Promise((r) => setTimeout(r, 15000));
  await step5_checkResponse();
}

main().catch((err) => {
  console.error("Error:", err);
  process.exit(1);
});
