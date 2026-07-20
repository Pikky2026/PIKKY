# Roadmap

Shipped milestones only. In-flight work lives in open issues, not here.

## Shipped

- [x] v0.1: antibody registry, one PDA per (kind, subject), genesis seeding
- [x] v0.2: stake / corroborate / challenge / resolve / slash economy in $WANE
- [x] v0.2: config-PDA-owned stake vault, fail-closed enforceability rule
- [x] v0.3: non-custodial session vault (enroll, deposit, screened execute)
- [x] v0.3: registry threat screen over CPI account-load, shared is_enforceable
- [x] v0.4: mandatory antibody-PDA binding (screen cannot be bypassed)
- [x] v0.4: vault withdraw + update_policy, registry update_config + governance
- [x] devnet deploy of registry and vault, live-verified clean/flagged/bypass
- [x] mainnet-beta deploy: registry `5Arj4z…1wJH`, vault `5YK7gM…rtTYh`, governor-owned config
- [x] TypeScript SDK: PDA derivation, instruction builders, both personas
- [x] litesvm end-to-end suite, 15 steps including bypass-rejection
- [x] devcontainer + multi-stage Dockerfile for reproducible builds
