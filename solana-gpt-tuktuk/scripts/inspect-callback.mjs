import { Connection } from '@solana/web3.js';
const connection = new Connection('https://api.devnet.solana.com', 'confirmed');

// The two most recent callbacks
const sigs = [
  '59XZajFjHeuVKF8Pxu12q8GjHb8SMWuZBqws4qRyBa9dgABxvLffWNFXQaib55VidGWWrfp5of8piokPtrmK273t', // 2026-05-15
  '4w84aYAfmGhSb5AnupB2AdZ7ZvMjwoGUxjq6fBh51poVXd3itTNErTn3dzm4ZQTYLmnBRujzrbwpBJhtGppRrQTX', // 2026-05-04
];

for (const sig of sigs) {
  console.log('\n=== Tx', sig.slice(0, 12), '===');
  const tx = await connection.getTransaction(sig, { maxSupportedTransactionVersion: 0 });
  if (!tx) { console.log('Not found'); continue; }
  console.log('Block time:', new Date(tx.blockTime * 1000).toISOString());
  console.log('Slot:', tx.slot, 'Fee:', tx.meta?.fee);
  console.log('Logs:');
  for (const l of (tx.meta?.logMessages || []).slice(0, 50)) console.log('  ', l);
}
