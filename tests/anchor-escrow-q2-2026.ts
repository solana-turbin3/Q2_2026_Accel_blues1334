import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import web3 from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getAssociatedTokenAddressSync,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import { BN } from "bn.js";
import { randomBytes } from "crypto";
import { expect } from "chai";
import { readFileSync } from "fs";
import path from "path";

const { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } = web3;

const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey(
  "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
);

const commitment: "processed" | "confirmed" | "finalized" = "confirmed";

describe("anchor-escrow-q2-2026", () => {
  const confirmTx = async (signature: string) => {
    const latestBlockhash = await anchor.getProvider().connection.getLatestBlockhash();
    await anchor.getProvider().connection.confirmTransaction(
      {
        signature,
        ...latestBlockhash,
      },
      commitment
    )
  }

  const confirmTxs = async (signatures: string[]) => {
    await Promise.all(signatures.map(confirmTx))
  }
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idlPath = path.join(process.cwd(), "target", "idl", "anchor_escrow_q2_2026.json");
  const programId = new PublicKey("d5Yda6GmqiHgyhf9skzY3ffFRu3Ku9hhqNEToPguytW");
  const idl = JSON.parse(readFileSync(idlPath, "utf8"));
  (idl as any).address = programId.toBase58();
  const program = new Program(idl, provider);

  const connection = provider.connection;
  
  const payer = provider.wallet as any;
  const taker = Keypair.generate();

  let mintA : web3.PublicKey;
  let mintB : web3.PublicKey;

  let makerAtaA: web3.PublicKey;
  let makerAtaB: web3.PublicKey;

  let takerAtaA: web3.PublicKey;
  let takerAtaB: web3.PublicKey;

  let vault: web3.PublicKey;

  const seed = new BN(randomBytes(8));

  const escrow = PublicKey.findProgramAddressSync([
    Buffer.from("escrow"), payer.publicKey.toBuffer(), seed.toBuffer("le", 8)
  ], programId)[0];

  it("Request airdrop to taker!", async () => {
    await Promise.all([payer, taker].map(async (k) => {

      // Request airdrop for the 'auth' account and confirm the transaction
      return await anchor.getProvider().connection.requestAirdrop(k.publicKey, 100 * anchor.web3.LAMPORTS_PER_SOL)
    })).then(confirmTxs);

  });

  it("Mint Tokens to Maker and Taker!", async () => {

    mintA = await createMint(
      connection,
      payer.payer,
      provider.publicKey,
      provider.publicKey,
      6,
    );

    console.log("mintA", mintA.toBase58());

    vault = getAssociatedTokenAddressSync(mintA, escrow, true);

    mintB = await createMint(
      connection,
      payer.payer,
      provider.publicKey,
      provider.publicKey,
      6,    
    );    
    console.log("mintB", mintB.toBase58());

    makerAtaA = (await getOrCreateAssociatedTokenAccount(
      connection,
      payer.payer,
      mintA,
      provider.publicKey,
    )).address;

    makerAtaB = (await getOrCreateAssociatedTokenAccount(
      connection,
      payer.payer,
      mintB,
      provider.publicKey,
    )).address;    

    takerAtaA = (await getOrCreateAssociatedTokenAccount(
      connection,
      payer.payer,
      mintA,
      taker.publicKey,
    )).address;

    takerAtaB = (await getOrCreateAssociatedTokenAccount(
      connection,
      payer.payer,
      mintB,
      taker.publicKey,
    )).address;


  await mintTo(
    connection,
    payer.payer,
    mintA,
    makerAtaA,
    payer.payer,
    1000_000_000,
  );
  console.log("tokens mints to makerataA", makerAtaA.toBase58());


  await mintTo(
    connection,
    payer.payer,
    mintB,
    takerAtaB,
    payer.payer,
    1000_000_000,
  );
  console.log("tokens mints to makerataB", makerAtaB.toBase58());

  });


  it("Make!", async () => {

    const initialMakerAtaABalance = await provider.connection.getTokenAccountBalance(makerAtaA);
    console.log("initial Maker Ata A balance", initialMakerAtaABalance.value.amount);

    const tx = await program.methods.make(
      seed,
      new BN(1_000_000),
      new BN(1_000_000),
    ).accountsStrict({
      maker: payer.publicKey,
      mintA: mintA,
      mintB: mintB,
      makerAtaA: makerAtaA,
      escrow: escrow,
      vault: vault,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

    await confirmTx(tx)

    const finalVaultBalance = await provider.connection.getTokenAccountBalance(vault);
    console.log("vault balance", finalVaultBalance.value.amount);
    const finalMakerAtaABalance = await provider.connection.getTokenAccountBalance(makerAtaA);
    console.log("Final Maker Ata A  balance", finalMakerAtaABalance.value.amount);
    console.log("make tx", tx);

  });

  it("Refund!", async () => {

    const refundSeed = new BN(randomBytes(8));
    const refundEscrow = PublicKey.findProgramAddressSync([
      Buffer.from("escrow"), payer.publicKey.toBuffer(), refundSeed.toBuffer("le", 8)
    ], program.programId)[0];
    const refundVault = getAssociatedTokenAddressSync(mintA, refundEscrow, true);

    const makeTx = await program.methods.make(
      refundSeed,
      new BN(1_000_000),
      new BN(1_000_000),
    ).accountsStrict({
      maker: payer.publicKey,
      mintA: mintA,
      mintB: mintB,
      makerAtaA: makerAtaA,
      escrow: refundEscrow,
      vault: refundVault,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

    await confirmTx(makeTx)

    const tx = await program.methods.refund(
    ).accountsPartial({
      maker: provider.publicKey,
      mintA: mintA,
      makerAtaA: makerAtaA,
      vault: refundVault,
      escrow: refundEscrow,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

    await confirmTx(tx)
    
    const vaultStateInfo = await provider.connection.getAccountInfo(refundVault);
    expect(vaultStateInfo).to.be.null;
    const escrowStateInfo = await provider.connection.getAccountInfo(refundEscrow);
    expect(escrowStateInfo).to.be.null;
    console.log("Refund tx", tx);
  });

  it("Take!", async () => {

    const tx = await program.methods.take(
    ).accountsPartial({
      taker: taker.publicKey,
      maker: provider.publicKey,
      mintA: mintA,
      mintB: mintB,
      vault: vault,
      makerAtaB: makerAtaB,
      takerAtaA: takerAtaA,
      takerAtaB: takerAtaB,
      escrow: escrow,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .signers([taker])
    .rpc();

    await confirmTx(tx)

    const vaultStateInfo = await provider.connection.getAccountInfo(vault);
    expect(vaultStateInfo).to.be.null;
    const escrowStateInfo = await provider.connection.getAccountInfo(escrow);
    expect(escrowStateInfo).to.be.null;
    console.log("Take tx", tx);
  });
});
