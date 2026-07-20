# Security Policy

## Supported Versions

| Version | Supported |
|---|---|
| 0.4.x | yes |
| 0.3.x | security fixes only |
| < 0.3 | no |

## Reporting a Vulnerability

If you believe you have found a vulnerability in the Wane engine, the
SDK, or any other artifact in this repository, please report it privately by
emailing `core@wane.network` with the following:

- A short description of the issue
- A minimal reproduction (Solana program logs, transaction signatures, or a
  test case that demonstrates the bug)
- Your assessment of the impact
- Whether you intend to disclose publicly and, if so, on what timeline

We will acknowledge your report within 72 hours and provide a public
disclosure timeline that does not put users at risk.

Please do not open a public GitHub issue for security reports.

## Scope

In scope for the bug bounty (live on mainnet-beta):

- Wane engine program (programs/wane)
- Reference subscriber program (programs/wane_vault)
- SDK (`@WaneProtocol/sdk`) signing or PDA-derivation paths
- CLI signed-transaction construction

Out of scope:

- Front-end (wane.network) issues unrelated to wallet interactions
- Issues that require physical access to a maintainer's device
- Social engineering of community members
- Issues already known and tracked in our internal queue

## Hardening Practices

- Engine program upgrades go through a multisig with a time-locked rotation.
- The engine cannot fabricate or redirect signals; every signal is persisted
  as an on-chain `SignalRecord` PDA with a monotonic id.
- Subscribers verify the engine's `GlobalState` PDA as the inbound CPI
  signer before reacting. See [`docs/cpi-contract.md`](./docs/cpi-contract.md).

## Coordinated Disclosure

We follow a 90-day coordinated disclosure window by default and reserve the
right to extend it for critical vulnerabilities that need user-side action
(e.g. a migration).
