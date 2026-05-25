import { PublicKey, Connection } from '@solana/web3.js';

// Default to Helius (same as crank-devnet.toml) — public devnet rate-limits hard
const DEFAULT_RPC = 'https://devnet.helius-rpc.com/?api-key=b4790339-6d47-45d8-b387-5a8e7e317695';
const connection = new Connection(process.env.RPC_URL || DEFAULT_RPC, 'confirmed');
const PROGRAM_ID = new PublicKey('9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt');
const ORACLE_PROGRAM_ID = new PublicKey('LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab');

const [gptConfig] = PublicKey.findProgramAddressSync([Buffer.from('gpt_config')], PROGRAM_ID);
const [payerPda] = PublicKey.findProgramAddressSync([Buffer.from('payer')], PROGRAM_ID);

const configInfo = await connection.getAccountInfo(gptConfig);
if (!configInfo) { console.log('GptConfig not found — run yarn test first'); process.exit(0); }
const contextAccount = new PublicKey(configInfo.data.subarray(40, 72));
const [interaction] = PublicKey.findProgramAddressSync(
  [Buffer.from('interaction'), payerPda.toBuffer(), contextAccount.toBuffer()],
  ORACLE_PROGRAM_ID
);

console.log('Watching for callback...');
console.log('  GptConfig:  ', gptConfig.toBase58());
console.log('  Interaction:', interaction.toBase58());
console.log('  (Ctrl+C to stop)\n');

let lastResponseLen = 0;
let tick = 0;

function readResponse(data) {
  let offset = 8 + 32 + 32;
  const promptLen = data.readUInt32LE(offset); offset += 4 + promptLen;
  const responseLen = data.readUInt32LE(offset); offset += 4;
  return data.subarray(offset, offset + responseLen).toString('utf-8');
}

setInterval(async () => {
  tick++;
  try {
    const [cfgInfo, ixInfo] = await Promise.all([
      connection.getAccountInfo(gptConfig),
      connection.getAccountInfo(interaction),
    ]);
    const response = cfgInfo ? readResponse(cfgInfo.data) : '';
    const isProcessed = ixInfo ? ixInfo.data[ixInfo.data.length - 1] === 1 : false;
    const stamp = new Date().toISOString().substring(11, 19);

    if (response && response.length > 0) {
      console.log(`[${stamp}] CALLBACK RECEIVED! response (${response.length} chars):\n`);
      console.log(response);
      console.log('\nDone. Run `yarn check` anytime to see it again.');
      process.exit(0);
    }

    process.stdout.write(`\r[${stamp}] tick #${tick}  is_processed=${isProcessed}  response="(empty)"     `);
  } catch (e) {
    process.stdout.write(`\r[${new Date().toISOString().substring(11,19)}] tick #${tick}  rpc error: ${e.message?.slice(0,80) ?? e}`);
  }
}, 30000);  // 30s — gentle on the shared Helius free tier
