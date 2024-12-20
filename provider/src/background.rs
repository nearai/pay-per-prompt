use std::time::Duration;

use futures::stream::{self, StreamExt};
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::{ProviderCtx, ProviderError};

const BATCH_SIZE: u32 = 16;
const MAX_CONCURRENT_TASKS: u32 = 4;
const STALE_CHANNEL_THRESHOLD: Duration = Duration::from_secs(60 * 60 * 24);

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
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                        match self.ctx.get_stale_channels(STALE_CHANNEL_THRESHOLD, Some(BATCH_SIZE)).await {
                            Ok(channels) => {
                                if !channels.is_empty() {
                                    info!("Found {} stale channels", channels.len());
                                    stream::iter(channels)
                                        .map(|channel| {
                                            let also_ctx = self.ctx.clone();
                                            let channel_name = channel.name.clone();
                                            async move {
                                                also_ctx.refresh_channel_row(&channel_name).await
                                            }
                                        })
                                        .buffer_unordered(MAX_CONCURRENT_TASKS as usize)
                                        .map(|result| match result {
                                            Ok(value) => {
                                                Some(value)
                                            }
                                            Err(err) => {
                                                error!("Error refreshing channel row: {:?}", err);
                                                None
                                            }
                                        })
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
