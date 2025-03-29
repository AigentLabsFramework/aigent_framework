# Aigent Framework

An open-source AI agent framework built on Solana.

## Overview

Aigent Framework is the core smart contract for a decentralized AI-driven agent system on the Solana blockchain. This contract serves as the foundation, allowing developers to understand, integrate, and build upon it to create their own monetizable AI agents and services.

We encourage developers to explore, suggest updates, and contribute improvements. While the core contract remains stable, the framework is designed for expansion and upgrades through community-driven development.

## Features

- **Escrow & Payments**
  - Securely lock funds (SOL or tokens) in escrow
  - Supports one-time and milestone-based transactions
  - Automates fund release with agent-driven logic

- **On-Chain Dispute Resolution**
  - Transparent and verifiable dispute resolution
  - Configurable rules for fair arbitration
  - AI-powered agents can mediate disputes

- **DAO Governance**
  - Community-driven decision-making through voting
  - Manage fees, rewards, and protocol upgrades
  - Fully decentralized control over ecosystem changes

- **Staking Mechanism**
  - Stake tokens to become an agent and earn rewards
  - Aligns incentives for participation and trust
  - Secure and transparent reward distribution

- **Monetization Hooks**
  - Transaction fees (SOL/tokens) as a revenue source
  - Staking rewards for active participants
  - Customizable payouts based on milestones

## Monetization & Expansion

Aigent Framework provides opportunities for developers, agents, and the community to monetize and expand upon its core features:

- **Developers & Builders**: Utilize the core contract to create decentralized AI agents, services, and applications.
- **Agents**: Connect AI-driven agents to facilitate escrow, dispute resolution, or milestone tracking.
- **Community dApps**: Build on top of Aigent Framework, leveraging its infrastructure while generating transaction-based revenue.
- **DAO-Led Evolution**: The framework can be improved through governance proposals, ensuring collective decision-making on future upgrades.

## Getting Started

### Prerequisites

Ensure you have the following tools installed before setting up the project:

- [Rust & Cargo](https://www.rust-lang.org/tools/install) – Required for compiling Solana programs
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools) – To interact with the Solana blockchain
- [Anchor Framework](https://www.anchor-lang.com/docs/installation) – For building smart contracts
- [Node.js](https://nodejs.org/en/download/) – Used for local testing and scripting

### Installation

```bash
# Clone the repository
git clone https://github.com/AigentLabsFramework/aigent_framework.git
cd aigent_framework

# Install dependencies
anchor build