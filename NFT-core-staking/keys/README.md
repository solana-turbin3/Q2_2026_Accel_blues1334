# keys/

Backup of the **program keypair** (`nft_core_staking-keypair.json`), whose public key
**is** the program address:

```
5ENTKY4nGmnwAvcEM3xnE8UeAaB4K3UFcgw995cfhwbS
```

It lives here (outside `target/`, which is gitignored) so a `cargo clean` or a moved folder
does not lose it. Anchor reads `target/deploy/nft_core_staking-keypair.json`; if that file is
missing, `anchor build` generates a *new* keypair and `anchor keys sync` would change the
program ID. To keep building/deploying against the existing on-chain program, restore it:

```bash
cp keys/nft_core_staking-keypair.json target/deploy/nft_core_staking-keypair.json
```

## Security

- This is the **program** keypair (the address identity). After deployment it grants **no**
  upgrade rights — upgrades are authorized by the separate **upgrade authority** wallet
  (`3njzSa5GMB7nPyP4xwKdmMS9KMhc7DF3yjHhcFG5YTSy`). So committing this devnet program keypair
  is low-risk.
- The **upgrade authority wallet** is the sensitive one. It is **not** in this repo and must
  **never** be committed. Keep it backed up separately.
- Do not reuse this exact keypair for a mainnet deployment — generate a fresh one there.
