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

Proto:
- UI: Show Mints in dialog
- Add mint!

MVP:
- Wallet init, seed verify
- cmd line args
- arg for DB file

Non-MVP:
- send EC from multiple mints, select automatically (feature on which level?)
- pending operations, show, check
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

