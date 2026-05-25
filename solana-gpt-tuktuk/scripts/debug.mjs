import { PublicKey, Connection } from '@solana/web3.js';
const connection = new Connection(process.env.RPC_URL || 'https://api.devnet.solana.com', 'confirmed');
const PROGRAM_ID = new PublicKey('9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt');
const ORACLE_PROGRAM_ID = new PublicKey('LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab');

const [gptConfig] = PublicKey.findProgramAddressSync([Buffer.from('gpt_config')], PROGRAM_ID);
const [payerPda] = PublicKey.findProgramAddressSync([Buffer.from('payer')], PROGRAM_ID);

// Read context_account from GptConfig
const configInfo = await connection.getAccountInfo(gptConfig);
if (!configInfo) {
  console.log('GptConfig not found — run `yarn test` to initialize.');
  process.exit(0);
}
const contextAccount = new PublicKey(configInfo.data.subarray(40, 72));

// Derive interaction PDA
const [interaction] = PublicKey.findProgramAddressSync(
  [Buffer.from('interaction'), payerPda.toBuffer(), contextAccount.toBuffer()],
  ORACLE_PROGRAM_ID
);

console.log('GptConfig:', gptConfig.toBase58());
console.log('Payer PDA:', payerPda.toBase58());
console.log('Context Account:', contextAccount.toBase58());
console.log('Interaction PDA:', interaction.toBase58());

// Check interaction account
const interactionInfo = await connection.getAccountInfo(interaction);
if (!interactionInfo) {
  console.log('\nInteraction account: NOT FOUND');
} else {
  console.log('\nInteraction account exists, size:', interactionInfo.data.length);
  console.log('Owner:', interactionInfo.owner.toBase58());

  // Parse interaction: 8 disc + 32 context + 32 user + 4+N text + 32 callback_program_id + 8 callback_disc + ...
  let offset = 8;
  const ctx = new PublicKey(interactionInfo.data.subarray(offset, offset + 32)); offset += 32;
  const user = new PublicKey(interactionInfo.data.subarray(offset, offset + 32)); offset += 32;
  const textLen = interactionInfo.data.readUInt32LE(offset); offset += 4;
  const text = interactionInfo.data.subarray(offset, offset + textLen).toString('utf-8'); offset += textLen;
  const callbackProgramId = new PublicKey(interactionInfo.data.subarray(offset, offset + 32)); offset += 32;
  const callbackDisc = interactionInfo.data.subarray(offset, offset + 8); offset += 8;

  // callback_account_metas: Vec<AccountMeta> = 4-byte len + entries
  const metasLen = interactionInfo.data.readUInt32LE(offset); offset += 4;
  const metas = [];
  for (let i = 0; i < metasLen; i++) {
    const pubkey = new PublicKey(interactionInfo.data.subarray(offset, offset + 32)); offset += 32;
    const isSigner = interactionInfo.data[offset] === 1; offset += 1;
    const isWritable = interactionInfo.data[offset] === 1; offset += 1;
    metas.push({ pubkey: pubkey.toBase58(), isSigner, isWritable });
  }

  const isProcessed = interactionInfo.data[offset] === 1;

  console.log('  context:', ctx.toBase58());
  console.log('  user (payer):', user.toBase58());
  console.log('  text:', text);
  console.log('  callback_program_id:', callbackProgramId.toBase58());
  console.log('  callback_discriminator:', Buffer.from(callbackDisc).toString('hex'));
  console.log('  callback_account_metas:', JSON.stringify(metas, null, 2));
  console.log('  is_processed:', isProcessed);
}

// Check payer PDA balance
const payerBalance = await connection.getBalance(payerPda);
console.log('\nPayer PDA balance:', payerBalance / 1e9, 'SOL');

// Check oracle identity PDA
const [oracleIdentity] = PublicKey.findProgramAddressSync([Buffer.from('identity')], ORACLE_PROGRAM_ID);
const identityInfo = await connection.getAccountInfo(oracleIdentity);
console.log('Oracle Identity PDA:', oracleIdentity.toBase58(), identityInfo ? 'EXISTS' : 'NOT FOUND');

// Check recent oracle txs for callback
console.log('\nChecking recent signatures on interaction account...');
const sigs = await connection.getSignaturesForAddress(interaction, { limit: 5 });
for (const s of sigs) {
  console.log(' ', s.signature, s.err ? 'FAILED' : 'OK', new Date(s.blockTime * 1000).toISOString());
}
