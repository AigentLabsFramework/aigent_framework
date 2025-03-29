# Contributing to Aigent Framework
We love pull requests—here’s how to join the party.

## Setup
- Rust: `rustup update`
- Solana CLI: `sh -c "$(curl -sSfL https://release.solana.com/stable/install)"`
- Anchor: `cargo install --git https://github.com/coral-xyz/anchor avm --locked`
- Clone & build: `git clone ... && cd aigent_framework && anchor build`

## How to Contribute
1. Fork it.
2. Branch it: `git checkout -b feat/your-cool-thing`
3. Code it—keep it Rust-y (4-space tabs, no tabs).
4. Test it: `anchor test`
5. PR it—link issues, describe changes.

## Rules
- No merge conflicts—rebase off `main`.
- Tests pass or you’re out.
- Be nice—check the [Code of Conduct](CODE_OF_CONDUCT.md).