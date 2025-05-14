# ParaKesh

Simple, reference implementation Cashu wallet, based on CDK.

## Features

- Receive Lightning
- Send Lightning
- Receive Ecash
- Send Ecash
- Add mint, select mint
- Store seed in an encrypted file (using [seedstore](https://github.com/optout21/seedstore))


## TODO

Proto:
- UI, RecLN: UI blocked while waiting for LN status
- UI: Mints in dialog
- Add mint!
MVP:
- iced upgrade to 13.1 (latest), wrapping
- Wallet init, seed verify
- cmd line args
- arg for DB file
Non-MVP:
- send EC from multiple mints, select automatically (feature on which level?)
- pending operations
- app: collect logs, provide
- claim pending
- list proofs
- re-mint, change denoms
- burn spent tokens
- send LN from multiple mints (MPP)

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


## MSRV

MSRV is Rust 1.85 (due to CDK, edition2024)

