use anchor_lang::prelude::*;
use mpl_core::instructions::CreateCollectionV2CpiBuilder;
use mpl_core::types::{
    Attribute, Attributes, Plugin, PluginAuthority, PluginAuthorityPair,
};

use crate::constants::*;
use crate::state::MplCore;

/// Create a Metaplex Core collection whose update authority is the program's
/// `update_authority` PDA. The collection is seeded with a `total_staked = 0`
/// Attributes plugin so it can track staking statistics from the start.
#[derive(Accounts)]
pub struct CreateCollection<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The collection account to create (a fresh keypair, must sign).
    #[account(mut)]
    pub collection: Signer<'info>,

    /// CHECK: PDA assigned as the collection's update authority. Used as a CPI
    /// signer for later plugin mutations.
    #[account(
        seeds = [UPDATE_AUTHORITY_SEED, collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    pub mpl_core_program: Program<'info, MplCore>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateCollection>, name: String, uri: String) -> Result<()> {
    let collection_stats = PluginAuthorityPair {
        plugin: Plugin::Attributes(Attributes {
            attribute_list: vec![Attribute {
                key: ATTR_TOTAL_STAKED.to_string(),
                value: "0".to_string(),
            }],
        }),
        // The update_authority PDA manages this plugin so stake/unstake can update it.
        authority: Some(PluginAuthority::UpdateAuthority),
    };

    CreateCollectionV2CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .collection(&ctx.accounts.collection.to_account_info())
        .payer(&ctx.accounts.payer.to_account_info())
        .update_authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .name(name)
        .uri(uri)
        .plugins(vec![collection_stats])
        .external_plugin_adapters(vec![])
        .invoke()?;

    Ok(())
}
