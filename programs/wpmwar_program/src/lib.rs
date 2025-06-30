use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Token, TokenAccount, Transfer as TokenTransfer};

declare_id!("6YnWTXBsLgFcEygmvA9r6FzVWu5gLxfgcpw4vXiQPUBJ");

#[program]
pub mod wpmwar_program {
    use super::*;

    /// ✅ Only admin can initialize global stats
    pub fn initialize_global_stats(ctx: Context<InitializeGlobalStats>) -> Result<()> {
        let stats = &mut ctx.accounts.global_stats;
        stats.total_players = 0;
        stats.total_matches = 0;
        Ok(())
    }

    pub fn join_match(ctx: Context<JoinMatch>, amount: u64) -> Result<()> {
        let game_room = &mut ctx.accounts.game_room;
        let player_profile = &mut ctx.accounts.player_profile;
        let global_stats = &mut ctx.accounts.global_stats;

        let ix = system_program::Transfer {
            from: ctx.accounts.player.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.to_account_info(), ix);
        system_program::transfer(cpi_ctx, amount)?;

        if player_profile.games_played == 0 && player_profile.wins == 0 {
            global_stats.total_players += 1;
        }
        player_profile.games_played += 1;

        if game_room.player1.is_none() {
            game_room.player1 = Some(*ctx.accounts.player.key);
            game_room.bet_amount = amount;
            game_room.status = 0;
        } else if game_room.player2.is_none() {
            game_room.player2 = Some(*ctx.accounts.player.key);
            game_room.status = 1;
            game_room.start_time = Clock::get()?.unix_timestamp; 
        } else {
            return err!(ErrorCode::RoomFull);
        }

        Ok(())
    }

    pub fn claim_reward(
        ctx: Context<ClaimReward>,
        winner: Pubkey,
        loser: Pubkey,
        wpm: u32,
    ) -> Result<()> {
        let game_room = &mut ctx.accounts.game_room;
        let global_stats = &mut ctx.accounts.global_stats;

        require!(game_room.status == 1, ErrorCode::MatchNotComplete);
        require!(
            winner == game_room.player1.unwrap() || winner == game_room.player2.unwrap(),
            ErrorCode::InvalidWinner
        );

        let winner_profile = &mut ctx.accounts.winner_profile;
        let loser_profile = &mut ctx.accounts.loser_profile;

        winner_profile.wins += 1;
        if wpm > winner_profile.highest_wpm {
            winner_profile.highest_wpm = wpm;
        }

        global_stats.total_matches += 1;

        let fee = game_room.bet_amount * 2 / 10;
        let payout = game_room.bet_amount * 2 - fee;

        let seeds: &[&[u8]] = &[b"vault-authority", &[ctx.bumps.vault_authority]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TokenTransfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.winner_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                &[seeds],
            ),
            payout,
        )?;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TokenTransfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.owner_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                &[seeds],
            ),
            fee,
        )?;

        game_room.status = 2;
        game_room.winner = Some(winner);

        Ok(())
    }

    pub fn forfeit_match(ctx: Context<ForfeitMatch>) -> Result<()> {
        let game_room = &mut ctx.accounts.game_room;

        require!(
            game_room.status == 0 || (game_room.status == 1 && game_room.player2.is_none()),
            ErrorCode::MatchStillActive
        );

        let now = Clock::get()?.unix_timestamp;
        if game_room.start_time > 0 {
            let elapsed = now - game_room.start_time;
            require!(elapsed > 600, ErrorCode::MatchStillActive);
        }

        let claimer = ctx.accounts.claimer.key();
        require!(
            claimer == game_room.player1.unwrap() || claimer == game_room.player2.unwrap(),
            ErrorCode::Unauthorized
        );

        let fee = game_room.bet_amount / 10;
        let payout = game_room.bet_amount - fee;

        // Transfer payout back to claimer
        let ix1 = system_program::Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.claimer.to_account_info(),
        };
        let cpi_ctx1 = CpiContext::new(ctx.accounts.system_program.to_account_info(), ix1);
        system_program::transfer(cpi_ctx1, payout)?;

        // Send fee to owner
        let ix2 = system_program::Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx2 = CpiContext::new(ctx.accounts.system_program.to_account_info(), ix2);
        system_program::transfer(cpi_ctx2, fee)?;

        game_room.status = 2;
        game_room.winner = Some(claimer);

        Ok(())
    }
}



#[derive(Accounts)]
pub struct InitializeGlobalStats<'info> {
    /// ✅ Only the admin can initialize
    #[account(mut, address = admin::ID)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + 16,
        seeds = [b"global_stats"],
        bump
    )]
    pub global_stats: Account<'info, GlobalStats>,

    pub system_program: Program<'info, System>,
}

// ✅ Replace with your actual admin pubkey
pub mod admin {
    use super::*;
    pub const ID: Pubkey = pubkey!("5YXqWPPLV36J8fvssCkwbrfFB5wYnJaTVvETef43apaW");
}

#[derive(Accounts)]
pub struct JoinMatch<'info> {
    #[account(mut)]
    pub player: Signer<'info>,

    #[account(mut)]
    pub vault: SystemAccount<'info>,

    #[account(
        init_if_needed,
        payer = player,
        space = 8 + 64,
        seeds = [b"profile", player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,

    #[account(
        init_if_needed,
        payer = player,
        space = 8 + 100,
        seeds = [b"game_room"],
        bump
    )]
    pub game_room: Account<'info, GameRoom>,

    #[account(
        mut,
        seeds = [b"global_stats"],
        bump
    )]
    pub global_stats: Account<'info, GlobalStats>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimReward<'info> {
    #[account(mut)]
    pub winner: Signer<'info>,

    #[account(mut)]
    pub winner_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        seeds = [b"vault-authority"],
        bump
    )]
    /// CHECK: The vault authority is a PDA that signs, so no extra validation is needed.
    /// CHECK: This is safe because it is only used as a signer for token transfers.
    pub vault_authority: AccountInfo<'info>,

    #[account(mut)]
    pub game_room: Account<'info, GameRoom>,

    #[account(mut, seeds = [b"profile", winner.key().as_ref()], bump)]
    pub winner_profile: Account<'info, PlayerProfile>,

    /// CHECK: Only used for seeds
    pub loser: AccountInfo<'info>,

    #[account(mut, seeds = [b"profile", loser.key().as_ref()], bump)]
    pub loser_profile: Account<'info, PlayerProfile>,

    #[account(mut, seeds = [b"global_stats"], bump)]
    pub global_stats: Account<'info, GlobalStats>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct GameRoom {
    pub player1: Option<Pubkey>,
    pub player2: Option<Pubkey>,
    pub bet_amount: u64,
    pub status: u8,
    pub winner: Option<Pubkey>,
    pub start_time: i64,  // NEW: when both players join
}


#[account]
pub struct PlayerProfile {
    pub games_played: u32,
    pub wins: u32,
    pub highest_wpm: u32,
}

#[account]
pub struct GlobalStats {
    pub total_players: u64,
    pub total_matches: u64,
}

#[derive(Accounts)]
pub struct ForfeitMatch<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,
    #[account(mut)]
    pub owner: SystemAccount<'info>,
    #[account(mut)]
    pub vault: SystemAccount<'info>,
    #[account(mut)]
    pub game_room: Account<'info, GameRoom>,
    pub system_program: Program<'info, System>,
}



#[error_code]
pub enum ErrorCode {
    #[msg("Room is full")]
    RoomFull,
    #[msg("Match not complete")]
    MatchNotComplete,
    #[msg("Match still active")]
    MatchStillActive,
    #[msg("Unauthorized forfeit")]
    Unauthorized,
    #[msg("Invalid winner")]
    InvalidWinner,
}

