# ParaKesh

Simple, reference implementation Cashu wallet, in Rust, with UI, based on CDK.

## Features

- Receive Lightning
- Send Lightning
- Receive Ecash
- Send Ecash
- Add mint, select mint
- Store seed in an encrypted file (using [seedstore](https://github.com/optout21/seedstore))


## TODO

Minor:
- No mint list, selected mint if only one mint

Proto:
- UI: Show Mints in dialog

MVP:
- mint onboarding: guide to adding mint, propose mints, links to pink
- Wallet init, seed verify
- cmd line args
- arg for DB file

Non-MVP:
- send EC from multiple mints, select automatically (feature on which level?)
- pending operations, show, check
- app: collect logs, provide
- claim pending
- mint list, with recommendations, etc.
- Parse and show info from entered LN invoice & cashu tokens
- list proofs
- re-mint, change denoms
- burn spent tokens
- send LN from multiple mints (MPP)
- read QR codes

CDK:
- melt_quote_status vs. mint_quote_state
- (Copy on CurrencyUnit, MintUrl)
- (Amount: as_sat, etc.)
- (melt_quote takes &str instead of String)


## Sample Mints

https://21mint.me
https://mint.lnwallet.app
https://mint.minibits.cash/Bitcoin


## Test Mints

https://testnut.cashu.space
https://fake.thesimplekid.dev
https://cashu.mutinynet.com  (on MutinyNet)


## MSRV

MSRV is Rust 1.85 (due to CDK, edition2024)

