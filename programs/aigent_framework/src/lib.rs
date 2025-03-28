#![cfg_attr(not(test), warn(unexpected_cfgs))]

use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};

declare_id!("Fku6SFoUXHcpVKfmW3CitF4sf9maYimg8msr1Xvyd3bD");

const MAX_DESC_LEN: usize = 1024;

pub mod helpers {
    use super::*;

    pub fn transfer_to_escrow<'info>(
        amount: u64,
        token_mint: Option<Pubkey>,
        from: &Signer<'info>,
        to: &Account<'info, Escrow>,
        token_program: &Program<'info, Token>,
        system_program: &Program<'info, System>,
        from_token: Option<&Account<'info, TokenAccount>>,
        to_token: Option<&Account<'info, TokenAccount>>,
    ) -> Result<()> {
        if let Some(mint) = token_mint {
            let from_acc = from_token.ok_or(ErrorCode::NoFunds)?;
            let to_acc = to_token.ok_or(ErrorCode::NoFunds)?;
            require!(from_acc.mint == mint && to_acc.mint == mint, ErrorCode::BadMint);
            require!(from_acc.amount >= amount, ErrorCode::NoFunds);
            anchor_spl::token::transfer(
                CpiContext::new(
                    token_program.to_account_info(),
                    anchor_spl::token::Transfer {
                        from: from_acc.to_account_info(),
                        to: to_acc.to_account_info(),
                        authority: from.to_account_info(),
                    },
                ),
                amount,
            )?;
        } else {
            require!(from.lamports() >= amount, ErrorCode::NoFunds);
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: from.to_account_info(),
                        to: to.to_account_info(),
                    },
                ),
                amount,
            )?;
        }
        Ok(())
    }

    pub fn transfer_funds<'info>(
        token_mint: Option<Pubkey>,
        escrow_account_info: &AccountInfo<'info>,
        to: &AccountInfo<'info>,
        amount: u64,
        token_program: &Program<'info, Token>,
        system_program: &Program<'info, System>,
        escrow_token: Option<&Account<'info, TokenAccount>>,
        to_token: Option<&Account<'info, TokenAccount>>,
        seeds: &[&[u8]],
    ) -> Result<()> {
        if let Some(mint) = token_mint {
            let from_acc = escrow_token.ok_or(ErrorCode::NoFunds)?;
            let to_acc = to_token.ok_or(ErrorCode::NoFunds)?;
            require!(from_acc.mint == mint && to_acc.mint == mint, ErrorCode::BadMint);
            anchor_spl::token::transfer(
                CpiContext::new_with_signer(
                    token_program.to_account_info(),
                    anchor_spl::token::Transfer {
                        from: from_acc.to_account_info(),
                        to: to_acc.to_account_info(),
                        authority: escrow_account_info.clone(),
                    },
                    &[seeds],
                ),
                amount,
            )?;
        } else {
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: escrow_account_info.clone(),
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
    use crate::helpers::{transfer_funds, transfer_to_escrow};

    const BPS_DENOM: u64 = 10_000;
    const MIN_AMOUNT: u64 = 10_000_000; // 0.01 SOL or equiv
    const MIN_FEE: u64 = 1_000_000; // 0.001 SOL
    const MAX_AMOUNT: u64 = 1_000_000_000; // 1B tokens
    const MAX_RELEASE: u64 = 31_536_000; // 1 year

    pub fn initialize_contract(
        ctx: Context<InitializeContract>,
        team_wallet: Pubkey,
        sol_fee_bps: u64,
        milestone_fee_bps: u64,
        req_token_amt: u64,
        token_mint: Pubkey,
    ) -> Result<()> {
        require!(sol_fee_bps <= BPS_DENOM, ErrorCode::BadFee);
        require!(milestone_fee_bps <= BPS_DENOM, ErrorCode::BadFee);
        require!(req_token_amt <= MAX_AMOUNT, ErrorCode::TooMuch);
        let config = &mut ctx.accounts.config;
        require!(config.authority == Pubkey::default(), ErrorCode::AlreadyInit);
        config.authority = *ctx.accounts.authority.key;
        config.team_wallet = team_wallet;
        config.sol_fee_bps = sol_fee_bps;
        config.milestone_fee_bps = milestone_fee_bps;
        config.required_token_amount = req_token_amt;
        config.token_mint = token_mint;
        Ok(())
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        team_wallet: Pubkey,
        sol_fee_bps: u64,
        milestone_fee_bps: u64,
        req_token_amt: u64,
        token_mint: Pubkey,
    ) -> Result<()> {
        require!(sol_fee_bps <= BPS_DENOM, ErrorCode::BadFee);
        require!(milestone_fee_bps <= BPS_DENOM, ErrorCode::BadFee);
        require!(req_token_amt <= MAX_AMOUNT, ErrorCode::TooMuch);
        let config = &mut ctx.accounts.config;
        require!(ctx.accounts.authority.key() == config.authority, ErrorCode::Unauthorized);
        config.team_wallet = team_wallet;
        config.sol_fee_bps = sol_fee_bps;
        config.milestone_fee_bps = milestone_fee_bps;
        config.required_token_amount = req_token_amt;
        config.token_mint = token_mint;
        Ok(())
    }

    pub fn initialize_escrow(
        ctx: Context<InitializeEscrow>,
        tx_id: Pubkey,
        amount: u64,
        release_secs: u64,
        token_mint: Option<Pubkey>,
    ) -> Result<()> {
        require!(amount >= MIN_AMOUNT, ErrorCode::TooLow);
        require!(amount <= MAX_AMOUNT, ErrorCode::TooMuch);
        require!(release_secs <= MAX_RELEASE, ErrorCode::TooLong);

        let escrow = &mut ctx.accounts.escrow;
        let now = Clock::get()?.unix_timestamp;
        escrow._transaction_id = tx_id;
        escrow.buyer = *ctx.accounts.buyer.key;
        escrow.seller = *ctx.accounts.seller.key;
        escrow.agent = *ctx.accounts.agent.key;
        escrow.amount = amount;
        escrow.is_disputed = false;
        escrow.is_completed = false;
        escrow.release_timestamp = now.checked_add(release_secs as i64).ok_or(ErrorCode::TimeOverflow)?;
        escrow.token_mint = token_mint;
        escrow.dispute_description = String::new();
        escrow.milestones = Vec::new();

        transfer_to_escrow(
            amount,
            token_mint,
            &ctx.accounts.buyer,
            escrow,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.buyer_token_account.as_ref(),
            ctx.accounts.escrow_token_account.as_ref(),
        )?;

        emit!(EscrowInitialized {
            transaction_id: escrow._transaction_id,
            buyer: escrow.buyer,
            seller: escrow.seller,
            agent: escrow.agent,
            amount,
            token_mint,
        });
        Ok(())
    }

    pub fn initialize_milestone_escrow(
        ctx: Context<InitializeMilestoneEscrow>,
        tx_id: Pubkey,
        milestones: Vec<Milestone>,
        token_mint: Option<Pubkey>,
    ) -> Result<()> {
        require!(!milestones.is_empty(), ErrorCode::NoMilestones);
        let total: u64 = milestones.iter().map(|m| m.amount).sum();
        require!(total >= MIN_AMOUNT, ErrorCode::TooLow);
        require!(total <= MAX_AMOUNT, ErrorCode::TooMuch);

        let escrow = &mut ctx.accounts.escrow;
        escrow._transaction_id = tx_id;
        escrow.buyer = *ctx.accounts.buyer.key;
        escrow.seller = *ctx.accounts.seller.key;
        escrow.agent = *ctx.accounts.agent.key;
        escrow.amount = total;
        escrow.is_disputed = false;
        escrow.is_completed = false;
        escrow.release_timestamp = 0;
        escrow.token_mint = token_mint;
        escrow.dispute_description = String::new();
        escrow.milestones = milestones;

        transfer_to_escrow(
            total,
            token_mint,
            &ctx.accounts.buyer,
            escrow,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.buyer_token_account.as_ref(),
            ctx.accounts.escrow_token_account.as_ref(),
        )?;

        emit!(MilestoneEscrowInitialized {
            transaction_id: escrow._transaction_id,
            buyer: escrow.buyer,
            seller: escrow.seller,
            agent: escrow.agent,
            milestones: escrow.milestones.clone(),
            token_mint,
        });
        Ok(())
    }

    pub fn release_payment(ctx: Context<ReleasePayment>, transaction_id: Pubkey) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        let config = &ctx.accounts.config;
        require!(ctx.accounts.agent.key() == escrow.agent, ErrorCode::Unauthorized);
        require!(!escrow.is_disputed, ErrorCode::Disputed);
        require!(!escrow.is_completed, ErrorCode::Done);
        require!(escrow.milestones.is_empty(), ErrorCode::HasMilestones);
    
        let fee = (escrow.amount * config.sol_fee_bps / BPS_DENOM)
            .max(if escrow.token_mint.is_none() { MIN_FEE } else { 0 });
        let seller_amt = escrow.amount.checked_sub(fee).ok_or(ErrorCode::Overflow)?;
        let seeds = &[b"escrow", transaction_id.as_ref(), &[ctx.bumps.escrow]];
    
        transfer_funds(
            escrow.token_mint,
            &escrow.to_account_info(),
            &ctx.accounts.team_wallet,
            fee,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.escrow_token_account.as_ref(),
            ctx.accounts.team_token_account.as_ref(),
            seeds,
        )?;
        transfer_funds(
            escrow.token_mint,
            &escrow.to_account_info(),
            &ctx.accounts.seller,
            seller_amt,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.escrow_token_account.as_ref(),
            ctx.accounts.seller_token_account.as_ref(),
            seeds,
        )?;
    
        escrow.is_completed = true;
        emit!(PaymentReleased {
            transaction_id: escrow._transaction_id,
            seller: escrow.seller,
            amount: seller_amt,
        });
        Ok(())
    }

    pub fn release_milestone(
        ctx: Context<ReleaseMilestone>,
        transaction_id: Pubkey,
        idx: u8,
    ) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        let config = &ctx.accounts.config;
        require!(ctx.accounts.agent.key() == escrow.agent, ErrorCode::Unauthorized);
        require!(!escrow.is_disputed, ErrorCode::Disputed);
        require!(!escrow.is_completed, ErrorCode::Done);
        require!(!escrow.milestones.is_empty(), ErrorCode::NoMilestones);

        let token_mint = escrow.token_mint;
        let escrow_account_info = escrow.to_account_info();
        let seeds = &[b"escrow", transaction_id.as_ref(), &[ctx.bumps.escrow]];
        let tx_id = escrow._transaction_id;

        let (milestone_amount, all_completed) = {
            let milestone = escrow.milestones.get_mut(idx as usize).ok_or(ErrorCode::BadIndex)?;
            require!(!milestone.is_completed, ErrorCode::MilestoneDone);
            let amount = milestone.amount;
            milestone.is_completed = true;
            let all_done = escrow.milestones.iter().all(|m| m.is_completed);
            (amount, all_done)
        };

        let fee = (milestone_amount * config.milestone_fee_bps / BPS_DENOM)
            .max(if token_mint.is_none() { MIN_FEE } else { 0 });
        let seller_amt = milestone_amount.checked_sub(fee).ok_or(ErrorCode::Overflow)?;

        transfer_funds(
            token_mint,
            &escrow_account_info,
            &ctx.accounts.team_wallet,
            fee,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.escrow_token_account.as_ref(),
            ctx.accounts.team_token_account.as_ref(),
            seeds,
        )?;
        transfer_funds(
            token_mint,
            &escrow_account_info,
            &ctx.accounts.seller,
            seller_amt,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.escrow_token_account.as_ref(),
            ctx.accounts.seller_token_account.as_ref(),
            seeds,
        )?;

        if all_completed {
            escrow.is_completed = true;
        }

        emit!(MilestoneReleased {
            transaction_id: tx_id,
            milestone_index: idx,
            amount: seller_amt,
        });

        Ok(())
    }

    pub fn check_and_release(ctx: Context<CheckAndRelease>, _transaction_id: Pubkey) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        let config = &ctx.accounts.config;
        let now = Clock::get()?.unix_timestamp;
        require!(now >= escrow.release_timestamp, ErrorCode::TooEarly);
        require!(!escrow.is_disputed, ErrorCode::Disputed);
        require!(!escrow.is_completed, ErrorCode::Done);
        require!(escrow.milestones.is_empty(), ErrorCode::HasMilestones);

        let fee = (escrow.amount * config.sol_fee_bps / BPS_DENOM)
            .max(if escrow.token_mint.is_none() { MIN_FEE } else { 0 });
        let seller_amt = escrow.amount.checked_sub(fee).ok_or(ErrorCode::Overflow)?;
        let seeds = &[b"escrow", _transaction_id.as_ref(), &[ctx.bumps.escrow]];

        transfer_funds(
            escrow.token_mint,
            &escrow.to_account_info(),
            &ctx.accounts.team_wallet,
            fee,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.escrow_token_account.as_ref(),
            ctx.accounts.team_token_account.as_ref(),
            seeds,
        )?;
        transfer_funds(
            escrow.token_mint,
            &escrow.to_account_info(),
            &ctx.accounts.seller,
            seller_amt,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.escrow_token_account.as_ref(),
            ctx.accounts.seller_token_account.as_ref(),
            seeds,
        )?;

        escrow.is_completed = true;
        emit!(PaymentReleased {
            transaction_id: escrow._transaction_id,
            seller: escrow.seller,
            amount: seller_amt,
        });
        Ok(())
    }

    pub fn start_dispute(
        ctx: Context<StartDispute>,
        _transaction_id: Pubkey,
        desc: String,
    ) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        let config = &ctx.accounts.config;
        require!(ctx.accounts.agent.key() == escrow.agent, ErrorCode::Unauthorized);
        require!(
            ctx.accounts.agent_token_account.amount >= config.required_token_amount,
            ErrorCode::LowTokens
        );
        require!(desc.len() <= MAX_DESC_LEN, ErrorCode::DescTooLong);
        require!(!desc.is_empty(), ErrorCode::DescTooShort);
        require!(!escrow.is_disputed, ErrorCode::AlreadyDisputed);
        require!(!escrow.is_completed, ErrorCode::Done);

        escrow.is_disputed = true;
        escrow.dispute_description = desc.clone();
        emit!(DisputeStarted {
            transaction_id: escrow._transaction_id,
            description: escrow.dispute_description.clone(),
        });
        Ok(())
    }

    pub fn resolve_dispute(
        ctx: Context<ResolveDispute>,
        _transaction_id: Pubkey,
        winner: Pubkey,
    ) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        let config = &ctx.accounts.config;
        require!(ctx.accounts.agent.key() == escrow.agent, ErrorCode::Unauthorized);
        require!(
            ctx.accounts.agent_token_account.amount >= config.required_token_amount,
            ErrorCode::LowTokens
        );
        require!(escrow.is_disputed, ErrorCode::NotDisputed);
        require!(!escrow.is_completed, ErrorCode::Done);
        require!(
            winner == escrow.buyer || winner == escrow.seller,
            ErrorCode::BadWinner
        );

        let fee_bps = if escrow.milestones.is_empty() {
            config.sol_fee_bps
        } else {
            config.milestone_fee_bps
        };
        let fee = (escrow.amount * fee_bps / BPS_DENOM)
            .max(if escrow.token_mint.is_none() { MIN_FEE } else { 0 });
        let winner_amt = escrow.amount.checked_sub(fee).ok_or(ErrorCode::Overflow)?;
        let seeds = &[b"escrow", _transaction_id.as_ref(), &[ctx.bumps.escrow]];

        transfer_funds(
            escrow.token_mint,
            &escrow.to_account_info(),
            &ctx.accounts.team_wallet,
            fee,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.escrow_token_account.as_ref(),
            ctx.accounts.team_token_account.as_ref(),
            seeds,
        )?;
        transfer_funds(
            escrow.token_mint,
            &escrow.to_account_info(),
            &ctx.accounts.winner,
            winner_amt,
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            ctx.accounts.escrow_token_account.as_ref(),
            ctx.accounts.winner_token_account.as_ref(),
            seeds,
        )?;

        escrow.is_completed = true;
        emit!(DisputeResolved {
            transaction_id: escrow._transaction_id,
            winner,
        });
        Ok(())
    }

    pub fn close_escrow(ctx: Context<CloseEscrow>, _transaction_id: Pubkey) -> Result<()> {
        let escrow = &ctx.accounts.escrow;
        let config = &ctx.accounts.config;
        require!(
            ctx.accounts.authority.key() == config.authority,
            ErrorCode::Unauthorized
        );
        require!(escrow.is_completed, ErrorCode::NotDone);
        Ok(())
    }
}

#[account]
pub struct Config {
    pub authority: Pubkey,
    pub team_wallet: Pubkey,
    pub sol_fee_bps: u64,
    pub milestone_fee_bps: u64,
    pub required_token_amount: u64,
    pub token_mint: Pubkey,
}

#[account]
pub struct Escrow {
    pub _transaction_id: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub agent: Pubkey,
    pub amount: u64,
    pub is_disputed: bool,
    pub is_completed: bool,
    pub release_timestamp: i64,
    pub token_mint: Option<Pubkey>,
    pub milestones: Vec<Milestone>,
    pub dispute_description: String,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Milestone {
    pub amount: u64,
    pub is_completed: bool,
    pub description: String,
}

#[derive(Accounts)]
pub struct InitializeContract<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 8 + 32 + 32 + 8 + 8 + 8 + 32)]
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
#[instruction(tx_id: Pubkey)]
pub struct InitializeEscrow<'info> {
    #[account(mut)]
    pub buyer: Signer<'info>,
    /// CHECK: Seller's wallet address; stored in escrow for later validation against subsequent instructions
    pub seller: AccountInfo<'info>,
    pub agent: Signer<'info>,
    #[account(
        init,
        payer = buyer,
        space = 8 + 32 * 4 + 8 + 1 + 1 + 8 + 33 + 4 + 4 + MAX_DESC_LEN,
        seeds = [b"escrow", tx_id.as_ref()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Buyer token account; validated in instruction logic if token_mint is provided
    #[account(mut)]
    pub buyer_token_account: Option<Account<'info, TokenAccount>>,
    /// CHECK: Escrow token account; validated in instruction logic if token_mint is provided
    #[account(mut)]
    pub escrow_token_account: Option<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
#[instruction(tx_id: Pubkey, milestones: Vec<Milestone>)]
pub struct InitializeMilestoneEscrow<'info> {
    #[account(
        init,
        payer = buyer,
        space = 8 + 32 * 4 + 8 + 1 + 1 + 8 + 33 + 4 + (milestones.len() * (8 + 1 + 4 + MAX_DESC_LEN)) + 4 + MAX_DESC_LEN,
        seeds = [b"escrow", tx_id.as_ref()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(mut)]
    pub buyer: Signer<'info>,
    /// CHECK: Seller's wallet address; stored in escrow for later validation against subsequent instructions
    pub seller: AccountInfo<'info>,
    pub agent: Signer<'info>,
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    /// CHECK: Buyer token account; validated in instruction logic if token_mint is provided
    #[account(mut)]
    pub buyer_token_account: Option<Account<'info, TokenAccount>>,
    /// CHECK: Escrow token account; validated in instruction logic if token_mint is provided
    #[account(mut)]
    pub escrow_token_account: Option<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
#[instruction(transaction_id: Pubkey)]
pub struct ReleasePayment<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    #[account(
        mut,
        seeds = [b"escrow", transaction_id.as_ref()],
        bump,
        has_one = agent @ ErrorCode::Unauthorized
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(mut, constraint = seller.key() == escrow.seller @ ErrorCode::Unauthorized)]
    /// CHECK: Seller's wallet address; stored in escrow for later validation against subsequent instructions
    pub seller: AccountInfo<'info>,
    #[account(mut, constraint = team_wallet.key() == config.team_wallet @ ErrorCode::Unauthorized)]
    /// CHECK: Team wallet address; stored in config for later validation
    pub team_wallet: AccountInfo<'info>,
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| escrow_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub escrow_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| seller_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub seller_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| team_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub team_token_account: Option<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
#[instruction(transaction_id: Pubkey, idx: u8)]
pub struct ReleaseMilestone<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    #[account(
        mut,
        seeds = [b"escrow", transaction_id.as_ref()],
        bump,
        has_one = agent @ ErrorCode::Unauthorized
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(mut, constraint = seller.key() == escrow.seller @ ErrorCode::Unauthorized)]
    /// CHECK: Seller's wallet address; stored in escrow for later validation against subsequent instructions
    pub seller: AccountInfo<'info>,
    #[account(mut, constraint = team_wallet.key() == config.team_wallet @ ErrorCode::Unauthorized)]
    /// CHECK: Team wallet address; stored in config for later validation
    pub team_wallet: AccountInfo<'info>,
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| escrow_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub escrow_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| seller_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub seller_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| team_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub team_token_account: Option<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
#[instruction(transaction_id: Pubkey)]
pub struct CheckAndRelease<'info> {
    #[account(
        mut,
        seeds = [b"escrow", transaction_id.as_ref()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(mut, constraint = seller.key() == escrow.seller @ ErrorCode::Unauthorized)]
    /// CHECK: Seller's wallet address; stored in escrow for later validation against subsequent instructions
    pub seller: AccountInfo<'info>,
    #[account(mut, constraint = team_wallet.key() == config.team_wallet @ ErrorCode::Unauthorized)]
    /// CHECK: Team wallet address; stored in config for later validation
    pub team_wallet: AccountInfo<'info>,
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| escrow_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub escrow_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| seller_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub seller_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| team_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub team_token_account: Option<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
#[instruction(transaction_id: Pubkey, desc: String)]
pub struct StartDispute<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    #[account(
        mut,
        seeds = [b"escrow", transaction_id.as_ref()],
        bump,
        has_one = agent @ ErrorCode::Unauthorized
    )]
    pub escrow: Account<'info, Escrow>,
    #[account(constraint = agent_token_account.mint == config.token_mint @ ErrorCode::BadMint)]
    pub agent_token_account: Account<'info, TokenAccount>,
    pub config: Account<'info, Config>,
}

#[derive(Accounts)]
#[instruction(transaction_id: Pubkey, winner: Pubkey)]
pub struct ResolveDispute<'info> {
    #[account(mut)]
    pub agent: Signer<'info>,
    #[account(
        mut,
        seeds = [b"escrow", transaction_id.as_ref()],
        bump,
        has_one = agent @ ErrorCode::Unauthorized
    )]
    pub escrow: Account<'info, Escrow>,
    /// CHECK: Winner's wallet address; validated in the function to be either buyer or seller
    #[account(mut)]
    pub winner: AccountInfo<'info>,
    #[account(mut, constraint = team_wallet.key() == config.team_wallet @ ErrorCode::Unauthorized)]
    /// CHECK: Team wallet address; stored in config for later validation
    pub team_wallet: AccountInfo<'info>,
    #[account(constraint = agent_token_account.mint == config.token_mint @ ErrorCode::BadMint)]
    pub agent_token_account: Account<'info, TokenAccount>,
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| escrow_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub escrow_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| winner_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub winner_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut, constraint = escrow.token_mint.map_or(true, |mint| team_token_account.mint == mint) @ ErrorCode::BadMint)]
    pub team_token_account: Option<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
#[instruction(transaction_id: Pubkey)]
pub struct CloseEscrow<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        mut,
        close = authority,
        seeds = [b"escrow", transaction_id.as_ref()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,
    pub config: Account<'info, Config>,
}

#[event]
pub struct EscrowInitialized {
    pub transaction_id: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub agent: Pubkey,
    pub amount: u64,
    pub token_mint: Option<Pubkey>,
}

#[event]
pub struct MilestoneEscrowInitialized {
    pub transaction_id: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub agent: Pubkey,
    pub milestones: Vec<Milestone>,
    pub token_mint: Option<Pubkey>,
}

#[event]
pub struct PaymentReleased {
    pub transaction_id: Pubkey,
    pub seller: Pubkey,
    pub amount: u64,
}

#[event]
pub struct MilestoneReleased {
    pub transaction_id: Pubkey,
    pub milestone_index: u8,
    pub amount: u64,
}

#[event]
pub struct DisputeStarted {
    pub transaction_id: Pubkey,
    pub description: String,
}

#[event]
pub struct DisputeResolved {
    pub transaction_id: Pubkey,
    pub winner: Pubkey,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Already disputed")]
    AlreadyDisputed,
    #[msg("Not disputed")]
    NotDisputed,
    #[msg("Escrow is disputed")]
    Disputed,
    #[msg("Already done")]
    Done,
    #[msg("Math overflow")]
    Overflow,
    #[msg("Time overflow")]
    TimeOverflow,
    #[msg("No funds")]
    NoFunds,
    #[msg("Bad winner")]
    BadWinner,
    #[msg("Too early")]
    TooEarly,
    #[msg("Milestone done")]
    MilestoneDone,
    #[msg("Not done")]
    NotDone,
    #[msg("Bad mint")]
    BadMint,
    #[msg("Desc too long")]
    DescTooLong,
    #[msg("Desc too short")]
    DescTooShort,
    #[msg("Too low")]
    TooLow,
    #[msg("Too much")]
    TooMuch,
    #[msg("Too long")]
    TooLong,
    #[msg("Bad fee")]
    BadFee,
    #[msg("Low tokens")]
    LowTokens,
    #[msg("Has milestones")]
    HasMilestones,
    #[msg("No milestones")]
    NoMilestones,
    #[msg("Bad index")]
    BadIndex,
    #[msg("Already init")]
    AlreadyInit,
}