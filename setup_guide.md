# Setup Guide
Get Aigent running locally.

## Prerequisites
- Rust 1.60+
- Solana CLI 1.18+
- Anchor 0.30+
- Node.js (for testing, optional)

## Steps
1. Install tools:
   ```bash
   rustup update
   sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
   cargo install --git https://github.com/coral-xyz/anchor avm --locked