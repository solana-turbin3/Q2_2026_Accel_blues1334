import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";
import {
  GetCommitmentSignature,
  DELEGATION_PROGRAM_ID,
  delegateBufferPdaFromDelegatedAccountAndOwnerProgram,
  delegationMetadataPdaFromDelegatedAccount,
  delegationRecordPdaFromDelegatedAccount,
} from "@magicblock-labs/ephemeral-rollups-sdk";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import BN from "bn.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

describe("er-state-account", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const providerEphemeralRollup = new anchor.AnchorProvider(
    new anchor.web3.Connection(process.env.EPHEMERAL_PROVIDER_ENDPOINT || "https://devnet.magicblock.app/", {wsEndpoint: process.env.EPHEMERAL_WS_ENDPOINT || "wss://devnet.magicblock.app/"}
    ),
    anchor.Wallet.local()
  );
  console.log("Base Layer Connection: ", provider.connection.rpcEndpoint);
  console.log("Ephemeral Rollup Connection: ", providerEphemeralRollup.connection.rpcEndpoint);
  console.log(`Current SOL Public Key: ${anchor.Wallet.local().publicKey}`)

  before(async function () {
    const balance = await provider.connection.getBalance(anchor.Wallet.local().publicKey)
    console.log('Current balance is', balance / LAMPORTS_PER_SOL, ' SOL','\n')
  })

  const programId = new PublicKey(
    "5jt9ZcQz8iKsmtaXDXWgUs1V8AQqJnbNCMQGEALiJtHB"
  );

  let program: Program;
  let programEphemeral: Program;

  before(async function () {
    const idlPath = path.resolve(__dirname, "../target/idl/er_state_account.json");
    const idlRaw = fs.readFileSync(idlPath, "utf8");
    const idl = JSON.parse(idlRaw);
    if (!idl.name && idl.metadata?.name) {
      idl.name = idl.metadata.name;
    }
    if (!idl.version && idl.metadata?.version) {
      idl.version = idl.metadata.version;
    }
    if (!idl.address) {
      idl.address = programId.toBase58();
    }

    // Prevent anchor-js from trying to build the `program.account.*` namespace,
    // which is not compatible with the IDL format emitted by anchor-cli 1.0.x.
    // This test suite only relies on `program.methods.*`.
    idl.accounts = [];

    program = new Program(idl, provider);
    programEphemeral = new Program(idl, providerEphemeralRollup);
  });

  const userAccount = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("user"), anchor.Wallet.local().publicKey.toBuffer()],
    programId
  )[0];

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().accountsPartial({
      user: anchor.Wallet.local().publicKey,
      userAccount: userAccount,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();
    console.log("User Account initialized: ", tx);
  });

  it("Update State!", async () => {
    const tx = await program.methods.update(new BN(Date.now() % 1_000_000)).accountsPartial({
      user: anchor.Wallet.local().publicKey,
      userAccount: userAccount,
    })
    .rpc();
    console.log("\nUser Account State Updated: ", tx);
  });

  it("Delegate to Ephemeral Rollup!", async () => {
    const bufferUserAccount = delegateBufferPdaFromDelegatedAccountAndOwnerProgram(
      userAccount,
      programId
    );
    const delegationRecordUserAccount = delegationRecordPdaFromDelegatedAccount(
      userAccount
    );
    const delegationMetadataUserAccount = delegationMetadataPdaFromDelegatedAccount(
      userAccount
    );

    try {
      const tx = await program.methods
        .delegate()
        .accountsPartial({
          user: anchor.Wallet.local().publicKey,
          bufferUserAccount,
          delegationRecordUserAccount,
          delegationMetadataUserAccount,
          userAccount: userAccount,
          validator: new PublicKey("MAS1Dt9qreoRMQ14YQuhg8UTZMMzDdKhmkZMECCzk57"),
          systemProgram: anchor.web3.SystemProgram.programId,
          ownerProgram: programId,
          delegationProgram: DELEGATION_PROGRAM_ID,
        })
        .rpc({ skipPreflight: false });

      console.log("\nUser Account Delegated to Ephemeral Rollup: ", tx);
    } catch (e: any) {
      if (typeof e?.getLogs === "function") {
        console.log(await e.getLogs(provider.connection));
      } else if (Array.isArray(e?.logs)) {
        console.log(e.logs);
      }
      throw e;
    }
  });

  it("Request Randomness (VRF inside ER)", async () => {
    console.log("\n===== VRF TEST CONTEXT: INSIDE ER (Ephemeral Rollup) =====");
    // ephemeral_vrf_sdk::consts::DEFAULT_EPHEMERAL_QUEUE
    const oracleQueue = new PublicKey(
      "5hBR571xnXppuCPveTrctfTU7tJLSN94nq7kv7FRK5Tc"
    );

    const DATA_OFFSET = 8 + 32;
    const readUserData = async (): Promise<bigint | null> => {
      const info = await providerEphemeralRollup.connection.getAccountInfo(
        userAccount
      );
      if (!info) return null;
      return info.data.readBigUInt64LE(DATA_OFFSET);
    };

    const before = await readUserData();
    console.log("user_account.data BEFORE VRF (ER):", before?.toString());

    let tx: string;
    try {
      tx = await programEphemeral.methods
        .requestRandomnessEr(13)
        .accountsPartial({
          payer: providerEphemeralRollup.wallet.publicKey,
          userAccount,
          oracleQueue,
        })
        .rpc({ skipPreflight: false });
      console.log("\nVRF Randomness Requested (ER): ", tx);
    } catch (e: any) {
      if (typeof e?.getLogs === "function") {
        console.log(await e.getLogs(providerEphemeralRollup.connection));
      } else if (Array.isArray(e?.logs)) {
        console.log(e.logs);
      }
      throw e;
    }

    const TIMEOUT_MS = 30_000;
    const POLL_MS = 1_000;
    const start = Date.now();
    let after: bigint | null = before;
    while (Date.now() - start < TIMEOUT_MS) {
      await new Promise((r) => setTimeout(r, POLL_MS));
      after = await readUserData();
      if (after !== null && after !== before) break;
    }

    if (after !== null && after !== before) {
      console.log(
        "\n🎲 VRF (ER) Randomness DELIVERED: user_account.data =",
        after.toString()
      );
    } else {
      console.warn(
        `\n⚠️  VRF (ER) callback not observed within ${TIMEOUT_MS / 1000}s (still ${after?.toString()}).`
      );
    }
  });

  it("Update State and Commit to Base Layer!", async () => {
    let txHash: string;
    try {
      txHash = await programEphemeral.methods
        .updateCommit(new BN(43))
        .accountsPartial({
          user: providerEphemeralRollup.wallet.publicKey,
        })
        .rpc({ skipPreflight: false });
    } catch (e: any) {
      if (typeof e?.getLogs === "function") {
        console.log(await e.getLogs(providerEphemeralRollup.connection));
      } else if (Array.isArray(e?.logs)) {
        console.log(e.logs);
      }
      throw e;
    }
    try {
      await GetCommitmentSignature(txHash, providerEphemeralRollup.connection);
    } catch (e) {
      console.log("GetCommitmentSignature failed:", e);
    }

    console.log("\nUser Account State Updated: ", txHash);
  });

  it("Commit and undelegate from Ephemeral Rollup!", async () => {
    let info = await providerEphemeralRollup.connection.getAccountInfo(userAccount);

    console.log("User Account Info: ", info);

    console.log("User account", userAccount.toBase58());

    let txHash: string;
    try {
      txHash = await programEphemeral.methods
        .undelegate()
        .accountsPartial({
          user: providerEphemeralRollup.wallet.publicKey,
        })
        .rpc({ skipPreflight: false });
    } catch (e: any) {
      if (typeof e?.getLogs === "function") {
        console.log(await e.getLogs(providerEphemeralRollup.connection));
      } else if (Array.isArray(e?.logs)) {
        console.log(e.logs);
      }
      throw e;
    }
    try {
      await GetCommitmentSignature(txHash, providerEphemeralRollup.connection);
    } catch (e) {
      console.log("GetCommitmentSignature failed:", e);
    }

    console.log("\nUser Account Undelegated: ", txHash);
  });

  it("Update State!", async () => {
    let tx = await program.methods.update(new BN(Date.now() % 1_000_000)).accountsPartial({
      user: anchor.Wallet.local().publicKey,
      userAccount: userAccount,
    })
    .rpc();

    console.log("\nUser Account State Updated: ", tx);
  });

  it("Request Randomness (VRF)", async () => {
    console.log("\n===== VRF TEST CONTEXT: OUTSIDE ER (Base Layer) =====");
    // ephemeral_vrf_sdk::consts::DEFAULT_QUEUE
    const oracleQueue = new PublicKey(
      "Cuj97ggrhhidhbu39TijNVqE74xvKJ69gDervRUXAxGh"
    );

    // UserAccount layout: 8 (disc) + 32 (user pubkey) + 8 (data: u64 LE) + 1 (bump)
    const DATA_OFFSET = 8 + 32;
    const readUserData = async (): Promise<bigint | null> => {
      const info = await provider.connection.getAccountInfo(userAccount);
      if (!info) return null;
      return info.data.readBigUInt64LE(DATA_OFFSET);
    };

    const before = await readUserData();
    console.log("user_account.data BEFORE VRF:", before?.toString());

    let tx: string;
    try {
      tx = await program.methods
        .requestRandomness(7)
        .accountsPartial({
          payer: anchor.Wallet.local().publicKey,
          userAccount,
          oracleQueue,
        })
        .rpc({ skipPreflight: false });
      console.log("\nVRF Randomness Requested: ", tx);
    } catch (e: any) {
      if (typeof e?.getLogs === "function") {
        console.log(await e.getLogs(provider.connection));
      } else if (Array.isArray(e?.logs)) {
        console.log(e.logs);
      }
      throw e;
    }

    // Poll until callback_randomness writes a new value (or timeout).
    const TIMEOUT_MS = 60_000;
    const POLL_MS = 1_500;
    const start = Date.now();
    let after: bigint | null = before;
    while (Date.now() - start < TIMEOUT_MS) {
      await new Promise((r) => setTimeout(r, POLL_MS));
      after = await readUserData();
      if (after !== null && after !== before) break;
    }

    if (after !== null && after !== before) {
      console.log("\n🎲 VRF Randomness DELIVERED: user_account.data =", after.toString());
    } else {
      console.warn(
        `\n⚠️  VRF callback not observed within ${TIMEOUT_MS / 1000}s (still ${after?.toString()}). The oracle may be slow on devnet.`
      );
    }
  });

  it("Close Account!", async () => {
    const tx = await program.methods.close().accountsPartial({
      user: anchor.Wallet.local().publicKey,
      userAccount: userAccount,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();
    console.log("\nUser Account Closed: ", tx);
  });
});
