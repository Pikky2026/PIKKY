use anchor_lang::prelude::*;

declare_id!("5Arj4zbFs5GigEGUSUb9hKNMYaPLqv1XgJXUcnGJ1wJH");

// Wane antibody registry, Solana port of WaneRegistry.sol — stake-free variant.
//
// Design note: the original registry priced every write with a $WANE stake and a
// challenge/slash game, so an open write surface could not be spammed. In the
// live product nobody stakes to report: users submit free leads and a gatekeeper
// bot verifies them off-chain, so the stake gate only blocked the bot with the
// project's own token and forced a token to exist before the registry could run.
// This variant removes staking entirely. Writes are governor-only: the governor
// (the verification bot / multisig) is the sole minter, having already done the
// verification the stake game used to price. Reading stays a client-side
// getAccountInfo, so reading is immunity with no view call and no token.

#[program]
pub mod wane_registry {
    use super::*;

    /// One-time setup. No mint, no stake vault: the registry has no token.
    pub fn init_config(ctx: Context<InitConfig>, p: InitParams) -> Result<()> {
        let c = &mut ctx.accounts.config;
        c.governor = ctx.accounts.governor.key();
        c.pending_governor = Pubkey::default();
        c.treasury = p.treasury;
        c.antibody_count = 0;
        c.enforce_window_secs = p.enforce_window_secs;
        c.enforce_corrobs = p.enforce_corrobs;
        c.genesis_open = true;
        c.paused = false;
        c.bump = ctx.bumps.config;
        Ok(())
    }

    /// Publish an antibody against (kind, subject). Governor-only: the bot mints
    /// what its off-chain verification has already confirmed. No stake, no token.
    /// The Antibody PDA address IS the dedup key: init fails if it already exists.
    pub fn mint_antibody(
        ctx: Context<MintAntibody>,
        kind: u8,
        subject: [u8; 32],
        evidence: [u8; 32],
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(!config.paused, WaneError::Paused);
        require!(kind <= 3, WaneError::BadKind);

        config.antibody_count = config.antibody_count.checked_add(1).unwrap();

        let now = Clock::get()?.unix_timestamp;
        let a = &mut ctx.accounts.antibody;
        a.id = config.antibody_count;
        a.kind = kind;
        a.status = Status::Active as u8;
        a.publisher = ctx.accounts.governor.key();
        a.minted_ts = now;
        a.corroborations = 0;
        a.subject = subject;
        a.evidence = evidence;
        a.bump = ctx.bumps.antibody;

        emit!(AntibodyMinted { id: a.id, kind, subject, publisher: a.publisher });
        Ok(())
    }

    /// Independently corroborate an existing antibody, hardening it faster than
    /// the maturity window. One vote per account (the Corroboration PDA enforces).
    pub fn corroborate(ctx: Context<Corroborate>) -> Result<()> {
        let a = &mut ctx.accounts.antibody;
        require!(a.status == Status::Active as u8, WaneError::NotActive);
        a.corroborations = a.corroborations.checked_add(1).unwrap();
        ctx.accounts.corroboration.bump = ctx.bumps.corroboration;
        Ok(())
    }

    /// Seed protocol-owned genesis antibodies (enforce immediately). Governor-only.
    pub fn seed_genesis(ctx: Context<SeedGenesis>, kind: u8, subject: [u8; 32]) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(config.genesis_open, WaneError::GenesisClosed);
        require!(kind <= 3, WaneError::BadKind);
        config.antibody_count = config.antibody_count.checked_add(1).unwrap();

        let now = Clock::get()?.unix_timestamp;
        let key = config.key();
        let a = &mut ctx.accounts.antibody;
        a.id = config.antibody_count;
        a.kind = kind;
        a.status = Status::Genesis as u8;
        a.publisher = key; // protocol-owned
        a.minted_ts = now;
        a.corroborations = 0;
        a.subject = subject;
        a.evidence = [0u8; 32];
        a.bump = ctx.bumps.antibody;
        Ok(())
    }

    /// Close the genesis window (governor only).
    pub fn close_genesis(ctx: Context<GovernorOnly>) -> Result<()> {
        ctx.accounts.config.genesis_open = false;
        Ok(())
    }

    /// Governor: revoke an antibody that turns out to be wrong. Replaces the old
    /// challenge/resolve game — with no public stake to arbitrate, correction is
    /// a direct governor action. A revoked antibody stops enforcing.
    pub fn revoke(ctx: Context<GovernorAntibody>) -> Result<()> {
        let a = &mut ctx.accounts.antibody;
        require!(a.status != Status::Revoked as u8, WaneError::NotActive);
        a.status = Status::Revoked as u8;
        Ok(())
    }

    /// Governor: update treasury + enforceability params. governor / counters
    /// are untouched.
    pub fn update_config(ctx: Context<GovernorOnly>, p: InitParams) -> Result<()> {
        let c = &mut ctx.accounts.config;
        c.treasury = p.treasury;
        c.enforce_window_secs = p.enforce_window_secs;
        c.enforce_corrobs = p.enforce_corrobs;
        Ok(())
    }

    /// Governor: pause / unpause the registry (blocks new mints).
    pub fn set_registry_paused(ctx: Context<GovernorOnly>, paused: bool) -> Result<()> {
        ctx.accounts.config.paused = paused;
        Ok(())
    }

    /// Governor: nominate a successor (two-step, takes effect on accept_governor).
    pub fn nominate_governor(ctx: Context<GovernorOnly>, new_governor: Pubkey) -> Result<()> {
        ctx.accounts.config.pending_governor = new_governor;
        Ok(())
    }

    /// Pending governor: accept the nomination and become governor.
    pub fn accept_governor(ctx: Context<AcceptGovernor>) -> Result<()> {
        let c = &mut ctx.accounts.config;
        require!(
            c.pending_governor != Pubkey::default()
                && ctx.accounts.new_governor.key() == c.pending_governor,
            WaneError::NotPending
        );
        c.governor = c.pending_governor;
        c.pending_governor = Pubkey::default();
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// Shared enforceability logic. The vault program ports the identical rule when
// it screens a send. Without stakes, a governor-minted antibody enforces once it
// matures OR collects enough corroborations; genesis enforces immediately;
// revoked never enforces.
// ----------------------------------------------------------------------------
impl Antibody {
    pub fn is_enforceable(&self, now: i64, enforce_window_secs: i64, enforce_corrobs: u32) -> bool {
        if self.status == Status::Revoked as u8 {
            return false;
        }
        if self.status == Status::Genesis as u8 {
            return true; // protocol-owned, trusted
        }
        if self.status != Status::Active as u8 {
            return false;
        }
        if self.corroborations >= enforce_corrobs {
            return true;
        }
        now >= self.minted_ts + enforce_window_secs
    }
}

// ---------------------------- state ----------------------------

#[account]
pub struct RegistryConfig {
    pub governor: Pubkey,
    pub pending_governor: Pubkey,
    pub treasury: Pubkey,
    pub antibody_count: u64,
    pub enforce_window_secs: i64,
    pub enforce_corrobs: u32,
    pub genesis_open: bool,
    pub paused: bool,
    pub bump: u8,
}
impl RegistryConfig {
    // 3 pubkeys + count u64 + window i64 + corrobs u32 + 2 bool + bump
    pub const LEN: usize = 32 * 3 + 8 + 8 + 4 + 1 + 1 + 1;
}

#[account]
pub struct Antibody {
    pub id: u64,
    pub kind: u8,
    pub status: u8,
    pub publisher: Pubkey,
    pub minted_ts: i64,
    pub corroborations: u32,
    pub subject: [u8; 32],
    pub evidence: [u8; 32],
    pub bump: u8,
}
impl Antibody {
    pub const LEN: usize = 8 + 1 + 1 + 32 + 8 + 4 + 32 + 32 + 1;
}

#[account]
pub struct Corroboration {
    pub bump: u8,
}

#[repr(u8)]
pub enum Status {
    None = 0,
    Active = 1,
    Revoked = 2,
    Genesis = 3,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitParams {
    pub treasury: Pubkey,
    pub enforce_window_secs: i64,
    pub enforce_corrobs: u32,
}

#[event]
pub struct AntibodyMinted {
    pub id: u64,
    pub kind: u8,
    pub subject: [u8; 32],
    pub publisher: Pubkey,
}

// ---------------------------- accounts ----------------------------

#[derive(Accounts)]
pub struct InitConfig<'info> {
    #[account(mut)]
    pub governor: Signer<'info>,
    #[account(
        init,
        payer = governor,
        space = 8 + RegistryConfig::LEN,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, RegistryConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(kind: u8, subject: [u8; 32])]
pub struct MintAntibody<'info> {
    #[account(mut, address = config.governor)]
    pub governor: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, RegistryConfig>,
    #[account(
        init,
        payer = governor,
        space = 8 + Antibody::LEN,
        seeds = [b"antibody".as_ref(), std::slice::from_ref(&kind), subject.as_ref()],
        bump
    )]
    pub antibody: Account<'info, Antibody>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Corroborate<'info> {
    #[account(mut)]
    pub corroborator: Signer<'info>,
    #[account(mut)]
    pub antibody: Account<'info, Antibody>,
    #[account(
        init,
        payer = corroborator,
        space = 8 + 1,
        seeds = [b"corrob", antibody.key().as_ref(), corroborator.key().as_ref()],
        bump
    )]
    pub corroboration: Account<'info, Corroboration>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(kind: u8, subject: [u8; 32])]
pub struct SeedGenesis<'info> {
    #[account(mut, address = config.governor)]
    pub governor: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, RegistryConfig>,
    #[account(
        init,
        payer = governor,
        space = 8 + Antibody::LEN,
        seeds = [b"antibody".as_ref(), std::slice::from_ref(&kind), subject.as_ref()],
        bump
    )]
    pub antibody: Account<'info, Antibody>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GovernorOnly<'info> {
    #[account(address = config.governor)]
    pub governor: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, RegistryConfig>,
}

#[derive(Accounts)]
pub struct GovernorAntibody<'info> {
    #[account(address = config.governor)]
    pub governor: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, RegistryConfig>,
    #[account(mut)]
    pub antibody: Account<'info, Antibody>,
}

#[derive(Accounts)]
pub struct AcceptGovernor<'info> {
    pub new_governor: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, RegistryConfig>,
}

// ---------------------------- errors ----------------------------

#[error_code]
pub enum WaneError {
    #[msg("registry is paused")]
    Paused,
    #[msg("bad threat kind")]
    BadKind,
    #[msg("antibody not active")]
    NotActive,
    #[msg("genesis window closed")]
    GenesisClosed,
    #[msg("no pending governor / mismatch")]
    NotPending,
}
