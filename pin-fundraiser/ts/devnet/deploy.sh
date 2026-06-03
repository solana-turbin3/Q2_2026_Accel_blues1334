#!/usr/bin/env bash
# Deploy the Pinocchio fundraiser program to devnet.
#
# The program derives PDAs and assigns account ownership from the `program_id`
# the runtime passes to the entrypoint, so it works at whatever address it is
# deployed to — no source patching needed. Just build and deploy.
#
# Usage:  ./devnet/deploy.sh
set -euo pipefail

# Resolve repo paths relative to this script.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROG_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
KEYPAIR="$PROG_DIR/target/deploy/pinocchio_fundraiser-keypair.json"

cd "$PROG_DIR"

# Build (cargo build-sbf also generates the program keypair on the first build).
cargo build-sbf

if [[ ! -f "$KEYPAIR" ]]; then
  echo "Generating program keypair..."
  solana-keygen new --no-bip39-passphrase -s -o "$KEYPAIR"
fi

PROGRAM_ID="$(solana address -k "$KEYPAIR")"
echo "Program id: $PROGRAM_ID"

echo "Deploying to devnet..."
solana program deploy \
  --url devnet \
  --program-id "$KEYPAIR" \
  "$PROG_DIR/target/deploy/pinocchio_fundraiser.so"

echo
echo "Deployed. Now run:"
echo "  export FUNDRAISER_PROGRAM_ID=$PROGRAM_ID"
echo "  npm run devnet"
