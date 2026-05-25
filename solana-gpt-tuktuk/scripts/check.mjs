import { PublicKey, Connection } from '@solana/web3.js';
const connection = new Connection(process.env.RPC_URL || 'https://api.devnet.solana.com', 'confirmed');
const PROGRAM_ID = new PublicKey('9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt');
const [gptConfig] = PublicKey.findProgramAddressSync([Buffer.from('gpt_config')], PROGRAM_ID);
const info = await connection.getAccountInfo(gptConfig);
if (info) {
  let offset = 8 + 32 + 32;
  const promptLen = info.data.readUInt32LE(offset); offset += 4;
  const prompt = info.data.subarray(offset, offset + promptLen).toString('utf-8'); offset += promptLen;
  const responseLen = info.data.readUInt32LE(offset); offset += 4;
  const response = info.data.subarray(offset, offset + responseLen).toString('utf-8');
  console.log('Prompt:', prompt);
  console.log('Response:', response || '(empty - oracle has not called back yet)');
} else {
  console.log('GptConfig not found');
}
