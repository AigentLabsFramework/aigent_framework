use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

// Yeah, this is our program's ID. Don't mess with it unless you know what you're doing.
declare_id!("4ufUzWyPvFqaS8VTdMcm973kgPthw9vHuzwUmBo8QnmJ");

// Max description length. 1024 bytes oughta be enough for anyone, right?
const MAX_DESC_LEN: usize = 1024;

// Helper functions to keep the main logic from turning into spaghetti.
pub mod helpers {
    use super::*;

    /// Sends funds to escrow—SOL or tokens, we handle both like champs.
    pub fn send_to_escrow<'info>(
        amount: u64,
        token_mint: Option<Pubkey>,
        from: &Signer<'info>,
        escrow: &AccountInfo<'info>,
        token_prog: &Program<'info, Token>,
        sys_prog: &Program<'info, System>,
        from_token: Option<&Account<'info, TokenAccount>>,
        escrow_token: Option<&Account<'info, TokenAccount>>,
    ) -> Result<()> {
        if let Some(mint) = token_mint {
            // Token transfer time. Make sure accounts match the mint or we’re screwed.
            let from_acc = from_token.ok_or(ProgramError::InvalidAccountData)?;
            let to_acc = escrow_token.ok_or(ProgramError::InvalidAccountData)?;
            if from_acc.mint != mint || to_acc.mint != mint {
                return Err(ProgramError::InvalidArgument.into());
            }
            token::transfer(
                CpiContext::new(
                    token_prog.to_account_info(),
                    token::Transfer {
                        from: from_acc.to_account_info(),
                        to: to_acc.to_account_info(),
                        authority: from.to_account_info(),
                    },
                ),
                amount,
            )?;
        } else {
            // Plain old SOL transfer. Simple, but don’t screw up the math.
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    sys_prog.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: from.to_account_info(),
                        to: escrow.to_account_info(),
                    },
                ),
                amount,
            )?;
        }
        Ok(())
    }

    /// Pulls funds out of escrow. Seeds make it secure, amount > 0 makes it worth doing.
    pub fn pull_funds<'info>(
        amount: u64,
        token_mint: Option<Pubkey>,
        escrow: &AccountInfo<'info>,
        to: &AccountInfo<'info>,
        token_prog: &Program<'info, Token>,
        sys_prog: &Program<'info, System>,
        escrow_token: Option<&Account<'info, TokenAccount>>,
        to_token: Option<&Account<'info, TokenAccount>>,
        seeds: &[&[u8]],
    ) -> Result<()> {
        if amount == 0 {
            // Why are we even here? Nothing to do.
            return Ok(());
        }
        if let Some(mint) = token_mint {
            let from_acc = escrow_token.ok_or(ProgramError::InvalidAccountData)?;
            let to_acc = to_token.ok_or(ProgramError::InvalidAccountData)?;
            if from_acc.mint != mint || to_acc.mint != mint {
                return Err(ProgramError::InvalidArgument.into());
            }
            token::transfer(
                CpiContext::new_with_signer(
                    token_prog.to_account_info(),
                    token::Transfer {
                        from: from_acc.to_account_info(),
                        to: to_acc.to_account_info(),
                        authority: escrow.clone(),
                    },
                    &[seeds],
                ),
                amount,
            )?;
        } else {
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    sys_prog.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: escrow.clone(),
                        to: to.to_account_info(),
                    },
                    &[seeds],
                ),
                amount,
            )?;
        }
        Ok(())
    }
}

#[program]
pub mod aigent_framework {
    use super::*;
    use crate::helpers::{pull_funds, send_to_escrow};

    // Basis points denominator. 10,000 bps = 100%. Math checks out.
    const BPS_DENOM: u64 = 10_000;

    /// Sets up the contract config. Authority, DAO pool, and fee—done.
    pub fn initialize_contract(
        ctx: Context<InitializeContract>,
        dao_pool: Pubkey,
        sol_fee_bps: u64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = *ctx.accounts.authority.key;
        config.dao_pool = dao_pool;
        config.sol_fee_bps = sol_fee_bps; // Fee in basis points, keep it reasonable.
        Ok(())
    }

    /// Kicks off an escrow. Buyer sends rent + deposit, we lock it up.
    pub fn start_escrow(
        ctx: Context<InitializeEscrow>,
        tx_id: Pubkey,
        rent: u64,
        deposit: u64,
        release_secs: u64,
        token_mint: Option<Pubkey>,
    ) -> Result<()> {
        let total = rent + deposit; // No overflow check—live dangerously.
        let meta = &mut ctx.accounts.transaction_metadata;
        let now = Clock::get()?.unix_timestamp;

        // Fill out the escrow ticket.
        meta._transaction_id = tx_id;
        meta.buyer = *ctx.accounts.buyer.key;
        meta.seller = *ctx.accounts.seller.key;
        meta.agent = *ctx.accounts.agent.key;
        meta.rental_amount = rent;
        meta.deposit_amount = deposit;
        meta.total_amount = total;
        meta.is_disputed = false;
        meta.is_completed = false;
        meta.release_timestamp = now + release_secs as i64;
        meta.token_mint = token_mint;
        meta.dispute_description = String::new();
        meta.deposit_release_status = DepositReleaseStatus::None;

        // Move the money. SOL or tokens, we don’t care.
        send_to_escrow(
            total,
            token_mint,
            &ctx.accounts.buyer,
            &ctx.accounts.central_sol_escrow,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.buyer_token_account.as_ref(),
            ctx.accounts.central_token_account.as_ref(),
        )?;
        Ok(())
    }

    /// Pays the rent to the seller, skims a fee for the DAO. Agent-only gig.
    pub fn pay_rent(ctx: Context<ReleaseRentalFee>, _tx_id: Pubkey) -> Result<()> {
        let meta = &mut ctx.accounts.transaction_metadata;
        let config = &ctx.accounts.config;

        if ctx.accounts.agent.key() != meta.agent {
            return Err(ProgramError::IllegalOwner.into()); // Nice try, impostor.
        }
        if meta.is_disputed {
            return Err(ProgramError::InvalidAccountData.into()); // Dispute’s on, hands off.
        }

        let fee = meta.rental_amount * config.sol_fee_bps / BPS_DENOM;
        let seller_cut = meta.rental_amount - fee;
        let seeds = &[b"central_sol_escrow".as_ref(), &[ctx.bumps.central_sol_escrow]];

        // DAO gets its cut.
        pull_funds(
            fee,
            meta.token_mint,
            &ctx.accounts.central_sol_escrow,
            &ctx.accounts.dao_pool,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.central_token_account.as_ref(),
            ctx.accounts.dao_pool_token_account.as_ref(),
            seeds,
        )?;
        // Seller gets the rest.
        pull_funds(
            seller_cut,
            meta.token_mint,
            &ctx.accounts.central_sol_escrow,
            &ctx.accounts.seller,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.central_token_account.as_ref(),
            ctx.accounts.seller_token_account.as_ref(),
            seeds,
        )?;
        Ok(())
    }

    /// Returns deposit to buyer—full or partial. Seller’s call.
    pub fn return_deposit(ctx: Context<ReleaseDeposit>, _tx_id: Pubkey, amount: u64) -> Result<()> {
        let meta = &mut ctx.accounts.transaction_metadata;

        if ctx.accounts.seller.key() != meta.seller {
            return Err(ProgramError::IllegalOwner.into()); // Not your deposit, pal.
        }
        if meta.is_disputed {
            return Err(ProgramError::InvalidAccountData.into()); // Dispute’s active, hold up.
        }
        if amount > meta.deposit_amount {
            return Err(ProgramError::InvalidArgument.into()); // Can’t give more than we got.
        }

        let seeds = &[b"central_sol_escrow".as_ref(), &[ctx.bumps.central_sol_escrow]];
        pull_funds(
            amount,
            meta.token_mint,
            &ctx.accounts.central_sol_escrow,
            &ctx.accounts.buyer,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.central_token_account.as_ref(),
            ctx.accounts.buyer_token_account.as_ref(),
            seeds,
        )?;

        if amount == meta.deposit_amount {
            meta.deposit_release_status = DepositReleaseStatus::Full;
            meta.is_completed = true; // Done and dusted.
        } else {
            let now = Clock::get()?.unix_timestamp;
            // Partial release, buyer gets 48 hours to cry foul.
            meta.deposit_release_status = DepositReleaseStatus::Partial {
                released_amount: amount,
                dispute_deadline: now + 48 * 3600, // 48 hours, close enough.
            };
        }
        Ok(())
    }

    /// Buyer disputes the deposit. Gotta describe why, though.
    pub fn dispute_deposit(ctx: Context<StartDepositDispute>, _tx_id: Pubkey, desc: String) -> Result<()> {
        let meta = &mut ctx.accounts.transaction_metadata;

        if ctx.accounts.buyer.key() != meta.buyer {
            return Err(ProgramError::IllegalOwner.into()); // Not your fight, buddy.
        }

        if let DepositReleaseStatus::Partial { dispute_deadline, .. } = meta.deposit_release_status {
            let now = Clock::get()?.unix_timestamp;
            if now >= dispute_deadline {
                return Err(ProgramError::InvalidAccountData.into()); // Too late, clock’s run out.
            }
        } else {
            return Err(ProgramError::InvalidAccountData.into()); // No partial release, no dispute.
        }

        meta.is_disputed = true;
        meta.dispute_description = desc; // Spill the tea.
        Ok(())
    }

    /// Agent settles a dispute. Splits the deposit between buyer and seller.
    pub fn settle_dispute(ctx: Context<ResolveDepositDispute>, _tx_id: Pubkey, renter_amt: u64, owner_amt: u64) -> Result<()> {
        let meta = &mut ctx.accounts.transaction_metadata;

        if ctx.accounts.agent.key() != meta.agent {
            return Err(ProgramError::IllegalOwner.into()); // Agent only, no randos.
        }
        if !meta.is_disputed {
            return Err(ProgramError::InvalidAccountData.into()); // Nothing to settle here.
        }

        let remaining = match meta.deposit_release_status {
            DepositReleaseStatus::Partial { released_amount, .. } => meta.deposit_amount - released_amount,
            _ => return Err(ProgramError::InvalidAccountData.into()), // Gotta be partial to settle.
        };
        if renter_amt + owner_amt != remaining {
            return Err(ProgramError::InvalidArgument.into()); // Math doesn’t add up, try again.
        }

        let seeds = &[b"central_sol_escrow".as_ref(), &[ctx.bumps.central_sol_escrow]];
        if renter_amt > 0 {
            pull_funds(
                renter_amt,
                meta.token_mint,
                &ctx.accounts.central_sol_escrow,
                &ctx.accounts.buyer,
                &ctx.accounts.token_program,
                &ctx.accounts.system_program,
                ctx.accounts.central_token_account.as_ref(),
                ctx.accounts.buyer_token_account.as_ref(),
                seeds,
            )?;
        }
        if owner_amt > 0 {
            pull_funds(
                owner_amt,
                meta.token_mint,
                &ctx.accounts.central_sol_escrow,
                &ctx.accounts.seller,
                &ctx.accounts.token_program,
                &ctx.accounts.system_program,
                ctx.accounts.central_token_account.as_ref(),
                ctx.accounts.seller_token_account.as_ref(),
                seeds,
            )?;
        }
        meta.is_completed = true; // Dispute’s over, move on.
        Ok(())
    }

    /// Auto-releases remaining deposit to seller if dispute deadline passes.
    pub fn auto_release(ctx: Context<AutoRelease>, _tx_id: Pubkey) -> Result<()> {
        let meta = &mut ctx.accounts.transaction_metadata;
        let now = Clock::get()?.unix_timestamp;

        let remaining = match meta.deposit_release_status {
            DepositReleaseStatus::Partial { dispute_deadline, released_amount } if now >= dispute_deadline => {
                meta.deposit_amount - released_amount
            }
            DepositReleaseStatus::Partial { .. } => return Err(ProgramError::InvalidAccountData.into()), // Too early.
            _ => return Err(ProgramError::InvalidAccountData.into()), // Nothing to release.
        };

        let seeds = &[b"central_sol_escrow".as_ref(), &[ctx.bumps.central_sol_escrow]];
        pull_funds(
            remaining,
            meta.token_mint,
            &ctx.accounts.central_sol_escrow,
            &ctx.accounts.seller,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.central_token_account.as_ref(),
            ctx.accounts.seller_token_account.as_ref(),
            seeds,
        )?;
        meta.is_completed = true; // Seller wins by default.
        Ok(())
    }
}

// Config account—keeps the big picture stuff.
#[account]
pub struct Config {
    pub authority: Pubkey,
    pub dao_pool: Pubkey,
    pub sol_fee_bps: u64,
}

// Transaction metadata—everything we need to track an escrow deal.
#[account]
pub struct TransactionMetadata {
    pub _transaction_id: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub agent: Pubkey,
    pub rental_amount: u64,
    pub deposit_amount: u64,
    pub total_amount: u64,
    pub is_disputed: bool,
    pub is_completed: bool,
    pub release_timestamp: i64,
    pub token_mint: Option<Pubkey>,
    pub deposit_release_status: DepositReleaseStatus,
    pub dispute_description: String,
}

// Enum for tracking deposit release state. Simple but effective.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum DepositReleaseStatus {
    None,
    Partial { released_amount: u64, dispute_deadline: i64 },
    Full,
}

// Context structs below—Anchor’s way of making sure we’ve got all the accounts we need.

// Setup the contract config.
#[derive(Accounts)]
pub struct InitializeContract<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + 32 + 32 + 8)]
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
}

// Start an escrow deal.
#[derive(Accounts)]
#[instruction(tx_id: Pubkey)]
pub struct InitializeEscrow<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    /// CHECK: Seller’s key, we trust it’s valid.
    pub seller: AccountInfo<'info>,
    pub agent: Signer<'info>,
    #[account(
        init,
        payer = buyer,
        space = 8 + 32 * 4 + 8 + 8 + 8 + 1 + 1 + 8 + 33 + 4 + MAX_DESC_LEN,
        seeds = [b"transaction", tx_id.as_ref()],
        bump
    )]
    pub transaction_metadata: Account<'info, TransactionMetadata>,
    /// CHECK: Escrow PDA, locked and loaded.
    #[account(mut, seeds = [b"central_sol_escrow"], bump)]
    pub central_sol_escrow: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub buyer_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub central_token_account: Option<Account<'info, TokenAccount>>,
}

// Release the rent payment.
#[derive(Accounts)]
#[instruction(transaction_id: Pubkey)]
pub struct ReleaseRentalFee<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    #[account(
        mut,
        seeds = [b"transaction", transaction_id.as_ref()],
        bump
    )]
    pub transaction_metadata: Account<'info, TransactionMetadata>,
    /// CHECK: Seller’s key, they’re getting paid.
    #[account(mut)]
    pub seller: AccountInfo<'info>,
    /// CHECK: DAO pool, skimming its fee.
    #[account(mut)]
    pub dao_pool: AccountInfo<'info>,
    pub config: Account<'info, Config>,
    /// CHECK: Escrow PDA, funds come from here.
    #[account(mut, seeds = [b"central_sol_escrow"], bump)]
    pub central_sol_escrow: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub central_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub seller_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub dao_pool_token_account: Option<Account<'info, TokenAccount>>,
}

// Return deposit to buyer.
#[derive(Accounts)]
#[instruction(transaction_id: Pubkey)]
pub struct ReleaseDeposit<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,
    #[account(
        mut,
        seeds = [b"transaction", transaction_id.as_ref()],
        bump
    )]
    pub transaction_metadata: Account<'info, TransactionMetadata>,
    /// CHECK: Buyer’s key, they’re getting cash back.
    #[account(mut)]
    pub buyer: AccountInfo<'info>,
    /// CHECK: Escrow PDA, funds are here.
    #[account(mut, seeds = [b"central_sol_escrow"], bump)]
    pub central_sol_escrow: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub central_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub buyer_token_account: Option<Account<'info, TokenAccount>>,
}

// Start a deposit dispute.
#[derive(Accounts)]
#[instruction(transaction_id: Pubkey, description: String)]
pub struct StartDepositDispute<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"transaction", transaction_id.as_ref()],
        bump
    )]
    pub transaction_metadata: Account<'info, TransactionMetadata>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

// Resolve a deposit dispute.
#[derive(Accounts)]
#[instruction(transaction_id: Pubkey)]
pub struct ResolveDepositDispute<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    #[account(
        mut,
        seeds = [b"transaction", transaction_id.as_ref()],
        bump
    )]
    pub transaction_metadata: Account<'info, TransactionMetadata>,
    /// CHECK: Buyer’s key, might get some funds.
    #[account(mut)]
    pub buyer: AccountInfo<'info>,
    /// CHECK: Seller’s key, might get some too.
    #[account(mut)]
    pub seller: AccountInfo<'info>,
    /// CHECK: Escrow PDA, holding the loot.
    #[account(mut, seeds = [b"central_sol_escrow"], bump)]
    pub central_sol_escrow: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub central_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub buyer_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub seller_token_account: Option<Account<'info, TokenAccount>>,
}

// Auto-release deposit to seller.
#[derive(Accounts)]
#[instruction(transaction_id: Pubkey)]
pub struct AutoRelease<'info> {
    #[account(
        mut,
        seeds = [b"transaction", transaction_id.as_ref()],
        bump
    )]
    pub transaction_metadata: Account<'info, TransactionMetadata>,
    /// CHECK: Seller’s key, they’re cashing in.
    #[account(mut)]
    pub seller: AccountInfo<'info>,
    /// CHECK: Escrow PDA, funds are here.
    #[account(mut, seeds = [b"central_sol_escrow"], bump)]
    pub central_sol_escrow: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub central_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub seller_token_account: Option<Account<'info, TokenAccount>>,
}