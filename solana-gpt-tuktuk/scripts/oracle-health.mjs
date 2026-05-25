import { PublicKey, Connection } from '@solana/web3.js';
const connection = new Connection('https://api.devnet.solana.com', 'confirmed');
const ORACLE_PROGRAM_ID = new PublicKey('LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab');
const [oracleIdentity] = PublicKey.findProgramAddressSync([Buffer.from('identity')], ORACLE_PROGRAM_ID);
const [oracleCounter] = PublicKey.findProgramAddressSync([Buffer.from('counter')], ORACLE_PROGRAM_ID);

console.log('Oracle Identity PDA:', oracleIdentity.toBase58());
console.log('Oracle Counter PDA:', oracleCounter.toBase58());

// Recent signatures on the oracle IDENTITY PDA = recent callbacks
console.log('\n=== Recent signatures on Oracle Identity PDA (these are callbacks) ===');
const idSigs = await connection.getSignaturesForAddress(oracleIdentity, { limit: 10 });
for (const s of idSigs) {
  const when = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : '?';
  console.log(' ', when, s.err ? 'FAILED' : 'OK', s.signature);
}

// Recent signatures on the COUNTER = new requests
console.log('\n=== Recent signatures on Oracle Counter PDA (new requests) ===');
const ctSigs = await connection.getSignaturesForAddress(oracleCounter, { limit: 10 });
for (const s of ctSigs) {
  const when = s.blockTime ? new Date(s.blockTime * 1000).toISOString() : '?';
  console.log(' ', when, s.err ? 'FAILED' : 'OK', s.signature);
}
