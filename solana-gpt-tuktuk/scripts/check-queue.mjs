import { Connection, PublicKey } from '@solana/web3.js';
const connection = new Connection('https://api.devnet.solana.com', 'confirmed');
const TUKTUK = new PublicKey('tuktukUrfhXT6ZT77QTU8RQtvgL967uRuVagWF57zVA');
const wallet = new PublicKey('3njzSa5GMB7nPyP4xwKdmMS9KMhc7DF3yjHhcFG5YTSy');
const suffix = wallet.toBase58().slice(0,6).toLowerCase();
const queueName = `gpt-tuktuk-${suffix}`;
const cronName = `ask-gpt-${suffix}`;
console.log('Expected queue name:', queueName);
console.log('Expected cron name:', cronName);
// Tuktuk task queue PDA: ["task_queue", name_bytes]
const [q] = PublicKey.findProgramAddressSync([Buffer.from('task_queue'), Buffer.from(queueName)], TUKTUK);
console.log('Queue PDA:', q.toBase58(), (await connection.getAccountInfo(q)) ? 'EXISTS' : 'NOT FOUND');
