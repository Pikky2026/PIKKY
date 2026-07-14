#!/usr/bin/env bash
# Wane Solana mainnet launch, one shot. Run in WSL (Ubuntu + Agave).
# Order: build -> deploy registry+vault -> init_config -> seed genesis.
# After this finishes, the Scan page returns real Solana verdicts.
#
# Prereqs:
#   - GOVERNOR_KEYPAIR points at a funded mainnet keypair (governor + payer)
#   - deploy keypairs exist in target/deploy/ (they do; ids match the SDK)
#   - ~2-4 SOL for program deploy rent
#
# Usage (WSL):
#   export GOVERNOR_KEYPAIR=/path/to/mainnet-gov.json
#   bash deploy-mainnet.sh
set -euo pipefail

export PATH=/root/.local/share/solana/install/active_release/bin:/root/.cargo/bin:/usr/local/bin:/usr/bin:/bin
export RUSTUP_TOOLCHAIN=1.96.0-x86_64-unknown-linux-gnu   # metadata cargo w/ edition2024

RPC="${SOLANA_RPC:-https://api.mainnet-beta.solana.com}"
: "${GOVERNOR_KEYPAIR:?set GOVERNOR_KEYPAIR to a funded mainnet keypair}"
REG_KP=target/deploy/wane_registry-keypair.json
VAULT_KP=target/deploy/wane_vault-keypair.json
REGISTRY=$(solana address -k "$REG_KP")
VAULT=$(solana address -k "$VAULT_KP")

echo "== Wane Solana mainnet launch =="
echo "registry: $REGISTRY"
echo "vault:    $VAULT"
solana config set --url "$RPC" >/dev/null
echo "payer/gov balance: $(solana balance -k "$GOVERNOR_KEYPAIR")"

echo "== 1/4 build (SBPFv3) =="
cargo-build-sbf --arch v3

echo "== 2/4 deploy programs =="
solana program deploy target/deploy/wane_registry.so \
  --program-id "$REG_KP" --keypair "$GOVERNOR_KEYPAIR" --url "$RPC"
solana program deploy target/deploy/wane_vault.so \
  --program-id "$VAULT_KP" --keypair "$GOVERNOR_KEYPAIR" --url "$RPC"

echo "== 3/4 init_config (governor = payer) =="
( cd /mnt/c/Users/baayo/Desktop/백혈구/base/bots/intel-sol \
  && SOLANA_RPC="$RPC" REGISTRY_PROGRAM="$REGISTRY" GOVERNOR_KEYPAIR="$GOVERNOR_KEYPAIR" \
     node dist/init.js )

echo "== 4/4 seed genesis from AllenHark blocklist =="
( cd /mnt/c/Users/baayo/Desktop/백혈구/base/bots/intel-sol \
  && SOLANA_RPC="$RPC" REGISTRY_PROGRAM="$REGISTRY" GOVERNOR_KEYPAIR="$GOVERNOR_KEYPAIR" \
     DRY_RUN=0 MAX_SEED="${MAX_SEED:-500}" node dist/seed.js )

echo "== DONE =="
echo "Now set on Vercel and redeploy:"
echo "  SOL_REGISTRY_PROGRAM=$REGISTRY"
echo "  NEXT_PUBLIC_SOLANA_RPC=$RPC"
echo "-> Scan returns real Solana verdicts; app drops the devnet badge."
