use anchor_lang::prelude::*;
use mpl_core::instructions::CreateV2CpiBuilder;

use crate::constants::*;
use crate::error::StakeError;
use crate::state::{BaseCollectionV1Wrap, MplCore};

/// Mint a Metaplex Core asset into the staking collection, owned by `user`.
/// The collection's update authority (the program PDA) signs as the creating
/// authority, so the new asset inherits the collection as its update authority.
#[derive(Accounts)]
pub struct MintAsset<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// The asset account to create (a fresh keypair, must sign).
    #[account(mut)]
    pub asset: Signer<'info>,

    #[account(
        mut,
        constraint = collection.update_authority == update_authority.key() @ StakeError::InvalidUpdateAuthority,
    )]
    pub collection: Account<'info, BaseCollectionV1Wrap>,

    /// CHECK: PDA update authority of the collection; signs the create CPI.
    #[account(
        seeds = [UPDATE_AUTHORITY_SEED, collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    pub mpl_core_program: Program<'info, MplCore>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<MintAsset>, name: String, uri: String) -> Result<()> {
    let collection_key = ctx.accounts.collection.key();
    let ua_signer_seeds: &[&[&[u8]]] = &[&[
        UPDATE_AUTHORITY_SEED,
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ]];

    CreateV2CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .payer(&ctx.accounts.user.to_account_info())
        .owner(Some(&ctx.accounts.user.to_account_info()))
        .update_authority(None)
        .system_program(&ctx.accounts.system_program.to_account_info())
        .name(name)
        .uri(uri)
        .plugins(vec![])
        .external_plugin_adapters(vec![])
        .invoke_signed(ua_signer_seeds)?;

    Ok(())
}
