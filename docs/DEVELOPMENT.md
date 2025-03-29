# Development Setup for Aigent Framework

This guide will help you set up your development environment for the Aigent Framework.

## Prerequisites
- Rust: Install Rust using ustup (https://rustup.rs/).
- Solana CLI: Follow the steps below to install Solana.
- Node.js: Install Node.js (https://nodejs.org/) for JavaScript dependencies.
- Anchor: Install Anchor CLI for Solana development (https://www.anchor-lang.com/docs/installation).

## Setting Up Solana
1. Install the Solana CLI:
   - On Windows, download and run the installer: curl https://release.solana.com/stable/install | sh.
   - Alternatively, use the provided install-solana.ps1 script in the repository.
2. Verify the installation:

   solana --version

3. Set up a local Solana validator (optional for development):

   solana-test-validator

## Building the Project
1. Navigate to the project directory:

   cd path/to/aigent_framework

2. Install JavaScript dependencies (if applicable):

   npm install

3. Build the project using Anchor:

   anchor build

## Running Tests
- Run tests using:

   anchor test

