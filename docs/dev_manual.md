markdown

# Aigent Framework Developer Manual

This manual is for developers integrating with the `aigent_framework` — a Solana program combining escrow for rental payments and a betting system, built with Anchor. Everything runs on **Devnet** with **SOL** (no SPL tokens needed yet), and **agents** play a central role in managing transactions.

---

## Overview

### Components
- **Escrow**: Manages rental agreements with buyers, sellers, and agents.
  - Locks rent + deposit
  - Releases funds with a DAO fee
  - Handles disputes
- **Betting**: Enables agents to create bets, bettors to wager, and winners to claim payouts.
  - Includes dispute resolution

### Key Features
- Uses **SOL** (`token_mint: None`) for all payments.
- **Agents** (wallets or bots) control key actions (e.g., releasing funds, declaring winners).
- **DAO pool** takes a fee from escrow rent payments.
- **PDAs** secure escrow and bet funds.

**Program ID (Devnet)**: `py5i9R6sU7xKej5WeWMNaiBcp9PtSpL13wnVLKpQxK5` (verify after deploy).

---

## Setup

### Prerequisites
- **Rust & Anchor**: Install via:
  ```sh
  curl https://sh.rustup.rs -sSf | sh
  cargo install --git https://github.com/coral-xyz/anchor anchor-cli

Solana CLI:
sh

sh -c "$(curl -sSfL https://release.solana.com/v1.18.4/install)"

Node.js: Required for client-side testing (@coral-xyz/anchor).

Deploy to Devnet
sh

git clone https://github.com/AigentLabsFramework/aigent_framework.git
cd aigent_framework && anchor build
anchor deploy --provider.cluster devnet
solana airdrop 2 <your-wallet> --url devnet

Note your deployed ProgramId if it changes.
Client Setup
Use Anchor's JavaScript client:
sh

npm init -y
npm install @coral-xyz/anchor @solana/web3.js

Load the IDL and connect:
typescript

import { Program, AnchorProvider } from '@coral-xyz/anchor';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';

const connection = new Connection('https://api.devnet.solana.com', 'confirmed');
const provider = new AnchorProvider(connection, wallet, {});
const program = new Program(idl, 'py5i9R6sU7xKej5WeWMNaiBcp9PtSpL13wnVLKpQxK5', provider);

Core Concepts
Accounts & PDAs
Config: Stores authority, dao_pool, and sol_fee_bps. Initialized once.

Escrow PDAs:
central_sol_escrow ([b"central_sol_escrow"]): Holds escrow funds.

bet_escrow ([b"bet_escrow", bet_id]): Holds bet pool.

TransactionMetadata ([b"transaction", tx_id]): Per-escrow state.

BetMetadata ([b"bet", bet_id]): Per-bet state.

SOL Usage
All functions use SOL (token_mint: None).

Token accounts are optional and null for now.

Agent Role
Escrow: Set in start_escrow, controls pay_rent, settle_dispute.

Betting: Set in create_bet, controls declare_winner, resolve_bet_dispute.

Can be a user’s wallet or a bot’s keypair.

Fee Structure
DAO Fee: rental_amount * sol_fee_bps / 10_000

Example: 5% fee at 500 bps.

Functions
Escrow Functions
initialize_contract
Sets up the framework.
Inputs: dao_pool: Pubkey, sol_fee_bps: u64.

Accounts: authority: Signer, config, system_program.

Call:

typescript

await program.methods.initializeContract(new PublicKey('YourDaoPool'), new BN(500))
  .accounts({ authority: wallet.publicKey, config: configPda, systemProgram: web3.SystemProgram.programId })
  .rpc();

start_escrow
Locks rent + deposit.
Inputs: tx_id: Pubkey, rent: u64, deposit: u64, release_secs: u64, token_mint: None.

Accounts: buyer: Signer, seller, agent: Signer, transaction_metadata, central_sol_escrow, system_program.

Call:

typescript

const txId = Keypair.generate().publicKey;
const [txPda] = PublicKey.findProgramAddressSync([Buffer.from("transaction"), txId.toBuffer()], program.programId);
const [escrowPda] = PublicKey.findProgramAddressSync([Buffer.from("central_sol_escrow")], program.programId);
await program.methods.startEscrow(txId, new BN(1_000_000), new BN(500_000), new BN(3600), null)
  .accounts({
    buyer: wallet.publicKey,
    seller: new PublicKey('SellerPubkey'),
    agent: wallet.publicKey,
    transactionMetadata: txPda,
    centralSolEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

pay_rent
Agent releases rent, DAO gets fee.
Inputs: tx_id: Pubkey.

Accounts: agent: Signer, seller, dao_pool, config, transaction_metadata, central_sol_escrow, system_program.

Call:

typescript

await program.methods.payRent(txId)
  .accounts({
    agent: agentWallet.publicKey,
    seller: sellerPubkey,
    daoPool: daoPoolPubkey,
    config: configPda,
    transactionMetadata: txPda,
    centralSolEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

confirm_receipt
Buyer releases rent instead of agent.
Inputs: tx_id: Pubkey.

Accounts: buyer: Signer, seller, dao_pool, config, transaction_metadata, central_sol_escrow, system_program.

Call:

typescript

await program.methods.confirmReceipt(txId)
  .accounts({
    buyer: wallet.publicKey,
    seller: sellerPubkey,
    daoPool: daoPoolPubkey,
    config: configPda,
    transactionMetadata: txPda,
    centralSolEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

return_deposit
Seller returns deposit (partial/full).
Inputs: tx_id: Pubkey, amount: u64.

Accounts: seller: Signer, buyer, transaction_metadata, central_sol_escrow, system_program.

Call:

typescript

await program.methods.returnDeposit(txId, new BN(500_000))
  .accounts({
    seller: wallet.publicKey,
    buyer: buyerPubkey,
    transactionMetadata: txPda,
    centralSolEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

dispute_deposit
Buyer disputes partial return.
Inputs: tx_id: Pubkey, desc: String.

Accounts: buyer: Signer, transaction_metadata, system_program.

Call:

typescript

await program.methods.disputeDeposit(txId, "Seller shorted me")
  .accounts({
    buyer: wallet.publicKey,
    transactionMetadata: txPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

settle_dispute
Agent splits remaining deposit.
Inputs: tx_id: Pubkey, renter_amt: u64, owner_amt: u64.

Accounts: agent: Signer, buyer, seller, transaction_metadata, central_sol_escrow, system_program.

Call:

typescript

await program.methods.settleDispute(txId, new BN(250_000), new BN(250_000))
  .accounts({
    agent: agentWallet.publicKey,
    buyer: buyerPubkey,
    seller: sellerPubkey,
    transactionMetadata: txPda,
    centralSolEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

auto_release
Auto-sends leftover deposit after 48 hours.
Inputs: tx_id: Pubkey.

Accounts: seller, transaction_metadata, central_sol_escrow, system_program.

Call:

typescript

await program.methods.autoRelease(txId)
  .accounts({
    seller: sellerPubkey,
    transactionMetadata: txPda,
    centralSolEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

Betting Functions
create_bet
Creates a bet with a pool.
Inputs: bet_id: Pubkey, description: String, options: Vec<BetOption>, max_payout: u64, token_mint: None.

Accounts: agent: Signer, bet_metadata, bet_escrow, system_program.

Call:

typescript

const betId = Keypair.generate().publicKey;
const [betPda] = PublicKey.findProgramAddressSync([Buffer.from("bet"), betId.toBuffer()], program.programId);
const [escrowPda] = PublicKey.findProgramAddressSync([Buffer.from("bet_escrow"), betId.toBuffer()], program.programId);
await program.methods.createBet(betId, "Team A vs B", [{ description: "Team A", odds: 2 }, { description: "Team B", odds: 3 }], new BN(5_000_000), null)
  .accounts({
    agent: wallet.publicKey,
    betMetadata: betPda,
    betEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

place_bet
Wagers on an option.
Inputs: bet_id: Pubkey, amount: u64, option_idx: u8.

Accounts: bettor: Signer, bet_metadata, bet_escrow, system_program.

Call:

typescript

await program.methods.placeBet(betId, new BN(1_000_000), 0)
  .accounts({
    bettor: wallet.publicKey,
    betMetadata: betPda,
    betEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

declare_winner
Agent sets winner.
Inputs: bet_id: Pubkey, winner_idx: u8.

Accounts: agent: Signer, bet_metadata, system_program.

Call:

typescript

await program.methods.declareWinner(betId, 0)
  .accounts({
    agent: agentWallet.publicKey,
    betMetadata: betPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

object_bet
Bettor disputes within 1 hour.
Inputs: bet_id: Pubkey.

Accounts: bettor: Signer, bet_metadata, system_program.

Call:

typescript

await program.methods.objectBet(betId)
  .accounts({
    bettor: wallet.publicKey,
    betMetadata: betPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

resolve_bet_dispute
Agent finalizes winner.
Inputs: bet_id: Pubkey, final_winner_idx: u8.

Accounts: agent: Signer, bet_metadata, system_program.

Call:

typescript

await program.methods.resolveBetDispute(betId, 0)
  .accounts({
    agent: agentWallet.publicKey,
    betMetadata: betPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

claim_winnings
Winners claim payout.
Inputs: bet_id: Pubkey.

Accounts: bettor: Signer, bet_metadata, bet_escrow, system_program.

Call:

typescript

await program.methods.claimWinnings(betId)
  .accounts({
    bettor: wallet.publicKey,
    betMetadata: betPda,
    betEscrow: escrowPda,
    systemProgram: web3.SystemProgram.programId,
  })
  .rpc();

Building Bots/Agents
Agent Responsibilities
Escrow: Call pay_rent or settle_dispute based on conditions.

Betting: Call declare_winner or resolve_bet_dispute using external data.

Bot Setup
Generate keypair:
sh

solana-keygen new -o agent.json

Fund it:
sh

solana airdrop 2 $(solana-keygen pubkey agent.json) --url devnet

Monitor state:
Escrow: program.account.transactionMetadata.fetch(txPda).

Betting: program.account.betMetadata.fetch(betPda).

Example Bot
typescript

import { Keypair } from '@solana/web3.js';

const agentKp = Keypair.fromSecretKey(loadFromFile('agent.json'));
const provider = new AnchorProvider(connection, new Wallet(agentKp), {});
const program = new Program(idl, programId, provider);

async function monitorEscrows() {
  const txs = await program.account.transactionMetadata.all();
  for (const tx of txs) {
    if (tx.account.agent.equals(agentKp.publicKey) && !tx.account.isDisputed) {
      if (Clock.get().unixTimestamp >= tx.account.releaseTimestamp) {
        await program.methods.payRent(tx.account._transactionId)
          .accounts({
            agent: agentKp.publicKey,
            seller: tx.account.seller,
            daoPool: configDaoPool,
            config: configPda,
            transactionMetadata: tx.publicKey,
            centralSolEscrow: escrowPda,
            systemProgram: web3.SystemProgram.programId,
          })
          .signers([agentKp])
          .rpc();
      }
    }
  }
}
setInterval(monitorEscrows, 60000);

Testing Workflows
Escrow Flow
initialize_contract (DAO pool, 500 bps).

start_escrow (1M rent, 500k deposit, 1-hour release).

pay_rent (agent releases, DAO gets 5%).

return_deposit (seller returns 500k).

(Optional) dispute_deposit → settle_dispute.

Betting Flow
create_bet (5M pool, 2 options).

place_bet (1M on option 0).

declare_winner (option 0 wins).

(Optional) object_bet → resolve_bet_dispute.

claim_winnings (2M payout at 2x odds).

Notes
Full IDL: target/idl/aigent_framework.json (post-build).

Report bugs: ../security.txt.

