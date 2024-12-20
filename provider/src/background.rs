use std::time::Duration;

use futures::stream::{self, StreamExt};
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::{ProviderCtx, ProviderError, STALE_CHANNEL_THRESHOLD};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH_SIZE: u32 = 16;
const MAX_CONCURRENT_TASKS: u32 = 4;
const CHANNEL_INACTIVITY_CLOSE_THRESHOLD: Duration = Duration::from_secs(60 * 60 * 24); // 1 day

pub struct ProviderBackgroundService {
    ctx: ProviderCtx,
}

impl ProviderBackgroundService {
    pub fn new(ctx: ProviderCtx) -> Self {
        Self { ctx }
    }

    pub fn run(self) -> JoinHandle<()> {
        let also_cancel_token = self.ctx.cancel_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = also_cancel_token.cancelled() => {
                        info!("Provider Background task shutting down.");
                        break;
                    }
                    _ = tokio::time::sleep(POLL_INTERVAL) => {
                        // The background task should
                        // 1. Withdraw+Close any inactive channels
                        // 2. Withdraw from any force closed channels

                        match self.ctx.db.get_stale_channels(STALE_CHANNEL_THRESHOLD, Some(BATCH_SIZE)).await {
                            Ok(channels) => {
                                if !channels.is_empty() {
                                    info!("Found {} stale channels", channels.len());
                                    stream::iter(channels)
                                        .map(|channel| {
                                            let also_ctx = self.ctx.clone();
                                            let channel_name = channel.name.clone();
                                            async move {
                                                let last_signed_state = match also_ctx.db.latest_signed_state(&channel_name).await {
                                                    Ok(Some(last_signed_state)) => last_signed_state,

                                                    // If no signed states are found then nothing to do
                                                    // Update the channel last active time, and return
                                                    Ok(None) => {
                                                        info!("No signed states found for stale channel {}", channel_name);
                                                        match also_ctx.db.update_channel_last_active(&channel_name).await {
                                                            Ok(_) => (),
                                                            Err(e) => error!("Error updating channel last active: {:?}", e),
                                                        };
                                                        return;
                                                    },

                                                    Err(ProviderError::DBError(e)) => {
                                                        error!("Database error getting latest signed state: {}", e);
                                                        return;
                                                    }
                                                    Err(e) => {
                                                        error!("Error getting latest signed state: {:?}", e);
                                                        return;
                                                    }
                                                };

                                                let channel_row = match also_ctx.db.refresh_channel_row(&channel_name).await {
                                                    Ok(channel_row) => channel_row,
                                                    Err(ProviderError::DBError(e)) => {
                                                        error!("Database error refreshing channel row: {}", e);
                                                        return;
                                                    }
                                                    Err(e) => {
                                                        error!("Error refreshing channel row: {:?}", e);
                                                        return;
                                                    }
                                                };

                                                let inactive_threshold = chrono::Utc::now().naive_utc() - CHANNEL_INACTIVITY_CLOSE_THRESHOLD;
                                                let has_withdrawable = channel_row.withdraw_balance() < last_signed_state.spent_balance();
                                                let channel_inactive = last_signed_state.created_at < inactive_threshold;

                                                if channel_inactive && has_withdrawable {
                                                    match also_ctx.try_withdraw_funds(&channel_row, &last_signed_state, true).await {
                                                        Ok(_) => (),
                                                        Err(e) => error!("Error withdrawing funds from channel {}: {:?}", channel_name, e),
                                                    }
                                                }

                                                if channel_row.force_close_started.is_some() && has_withdrawable {
                                                    match also_ctx.try_withdraw_funds(&channel_row, &last_signed_state, false).await {
                                                        Ok(_) => (),
                                                        Err(e) => error!("Error withdrawing funds from channel {}: {:?}", channel_name, e),
                                                    }
                                                }

                                            }
                                        })
                                        .buffer_unordered(MAX_CONCURRENT_TASKS as usize)
                                        .collect::<Vec<_>>()
                                        .await;
                                }
                            }
                            Err(ProviderError::DBError(e)) => {
                                error!("Database error getting stale channels: {}", e);
                            }
                            Err(e) => {
                                error!("Error getting stale channels: {:?}", e);
                            }
                        }
                    }
                }
            }
        })
    }
}
