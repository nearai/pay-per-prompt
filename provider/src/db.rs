use std::{str::FromStr, time::Duration};

use chrono::Utc;
use cli::{
    config::{SignedState, State},
    contract::ContractChannel,
};
use near_crypto::Signature;
use near_sdk::{AccountId, NearToken};
use sqlx::sqlite::SqlitePool;
use tracing::{error, info, warn};

use crate::{
    ChannelError, ProviderError, ProviderResult, CLOSED_CHANNEL_ACCOUNT_ID, STALE_CHANNEL_THRESHOLD,
};

#[derive(Default, Debug, sqlx::FromRow)]
pub struct ChannelRow {
    pub id: i64,
    pub updated_at: chrono::NaiveDateTime,
    pub name: String,
    pub receiver: String,
    pub receiver_pk: String,
    pub sender: String,
    pub sender_pk: String,
    // Near Tokens in contracts are represented as u128's
    // this isn't supported by sqlite, so we store them as bytes big endian
    pub added_balance: Vec<u8>,
    pub withdrawn_balance: Vec<u8>,

    pub force_close_started: Option<chrono::NaiveDateTime>,
    pub soft_closed: bool,
}

impl ChannelRow {
    pub fn added_balance(&self) -> NearToken {
        NearToken::from_yoctonear(u128::from_be_bytes(
            self.added_balance[..].try_into().unwrap_or([0; 16]),
        ))
    }

    pub fn withdrawn_balance(&self) -> NearToken {
        NearToken::from_yoctonear(u128::from_be_bytes(
            self.withdrawn_balance[..].try_into().unwrap_or([0; 16]),
        ))
    }

    pub fn is_closed(&self) -> bool {
        self.receiver == CLOSED_CHANNEL_ACCOUNT_ID && self.sender == CLOSED_CHANNEL_ACCOUNT_ID
    }

    pub fn is_closing(&self) -> bool {
        self.force_close_started.is_some()
    }

    pub fn is_stale(&self) -> bool {
        let now = Utc::now().naive_utc();
        let inactive_threshold = now - STALE_CHANNEL_THRESHOLD;
        self.updated_at < inactive_threshold
    }

    pub fn as_closed_result(&self) -> ProviderResult<()> {
        let also_name = self.name.to_owned();
        if self.force_close_started.is_some() {
            return Err(ProviderError::Channel(ChannelError::Closing(also_name)));
        }
        if self.is_closed() {
            return Err(ProviderError::Channel(ChannelError::HardClosed(also_name)));
        }
        if self.soft_closed {
            return Err(ProviderError::Channel(ChannelError::SoftClosed(also_name)));
        }
        Ok(())
    }
}

#[derive(Default, Debug, sqlx::FromRow)]
pub struct SignedStateRow {
    pub id: i64,
    pub created_at: sqlx::types::chrono::NaiveDateTime,
    pub channel_id: i64,
    pub spent_balance: Vec<u8>,
    pub signature: String,
}

impl SignedStateRow {
    pub fn spent_balance(&self) -> NearToken {
        NearToken::from_yoctonear(u128::from_be_bytes(
            self.spent_balance[..].try_into().unwrap_or([0; 16]),
        ))
    }

    pub async fn as_signed_state(&self, db: &ProviderDb) -> ProviderResult<SignedState> {
        let channel = db.get_channel_from_signed_state(self).await?;
        Ok(SignedState {
            state: State {
                channel_id: channel.name,
                spent_balance: self.spent_balance(),
            },
            signature: Signature::from_str(&self.signature).unwrap(),
        })
    }
}

#[derive(Clone)]
pub struct ProviderDb {
    connection: SqlitePool,
    account_id: AccountId,
}

impl ProviderDb {
    pub fn new(database_url: &str, account_id: AccountId) -> Self {
        info!("Initializing database");
        let also_database_url = database_url.to_string();
        let result = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let connection = SqlitePool::connect(&also_database_url).await.unwrap();
                connection
            })
        });
        let connection = result.join().expect("Thread panicked");

        Self {
            connection,
            account_id,
        }
    }

    pub async fn get_channel_row(&self, channel_name: &str) -> ProviderResult<ChannelRow> {
        match sqlx::query_as!(
            ChannelRow,
            "SELECT * FROM channel WHERE name = ? LIMIT 1",
            channel_name
        )
        .fetch_optional(&self.connection)
        .await
        {
            Ok(Some(channel)) => {
                info!("Querying channel retrieved from database: {}", channel_name);
                Ok(channel)
            }
            Ok(None) | Err(sqlx::Error::RowNotFound) => {
                warn!("Querying channel not found in database: {}", channel_name);
                Err(ProviderError::Channel(ChannelError::NotFoundInDB))
            }
            Err(e) => {
                error!("Error querying channel from database: {}", e);
                Err(ProviderError::DBError(e))
            }
        }
    }

    pub async fn upsert_channel_row(
        &self,
        channel_name: &str,
        contract_channel: ContractChannel,
    ) -> ProviderResult<ChannelRow> {
        let sender_account = contract_channel.sender.account_id.to_string();
        let sender_pk = contract_channel.sender.public_key.to_string();
        let receiver_account = contract_channel.receiver.account_id.to_string();
        let receiver_pk = contract_channel.receiver.public_key.to_string();
        let added_balance = contract_channel
            .added_balance
            .as_yoctonear()
            .to_be_bytes()
            .to_vec();
        let withdrawn_balance = contract_channel
            .withdrawn_balance
            .as_yoctonear()
            .to_be_bytes()
            .to_vec();

        info!("Upserting channel into database: {}", channel_name);
        let contract_channel_row = sqlx::query_as!(
            ChannelRow,
            r#"
            INSERT INTO channel
            (name, sender, sender_pk, receiver, receiver_pk, added_balance, withdrawn_balance)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                updated_at = CURRENT_TIMESTAMP,
                sender = excluded.sender,
                sender_pk = excluded.sender_pk,
                receiver = excluded.receiver,
                receiver_pk = excluded.receiver_pk,
                added_balance = excluded.added_balance,
                withdrawn_balance = excluded.withdrawn_balance,
                updated_at = CURRENT_TIMESTAMP
            RETURNING *
            "#,
            channel_name,
            sender_account,
            sender_pk,
            receiver_account,
            receiver_pk,
            added_balance,
            withdrawn_balance
        )
        .fetch_one(&self.connection)
        .await;

        contract_channel_row.map_err(|e| {
            error!("Error upserting channel into database: {}", e);
            ProviderError::DBError(e)
        })
    }

    pub async fn update_channel_last_active(
        &self,
        channel_name: &str,
    ) -> ProviderResult<ChannelRow> {
        let updated_channel_row = sqlx::query_as!(
            ChannelRow,
            r#"
            UPDATE channel
            SET updated_at = CURRENT_TIMESTAMP
            WHERE name = ?
            RETURNING *
            "#,
            channel_name
        )
        .fetch_optional(&self.connection)
        .await;

        updated_channel_row
            .map_err(|e| {
                error!("Error updating channel in database: {}", e);
                ProviderError::DBError(e)
            })?
            .ok_or(ProviderError::Channel(ChannelError::NotFoundInDB))
    }

    pub async fn insert_signed_state(
        &self,
        signed_state: &SignedState,
    ) -> ProviderResult<SignedStateRow> {
        let channel_row = self.get_channel_row(&signed_state.state.channel_id).await?;
        channel_row.as_closed_result()?;

        let spent_balance = signed_state
            .state
            .spent_balance
            .as_yoctonear()
            .to_be_bytes()
            .to_vec();
        let signature = signed_state.signature.to_string();
        info!(
            "Inserting new latest signed state for channel {} into database",
            channel_row.name
        );
        let signed_state_row = sqlx::query_as!(
            SignedStateRow,
            r#"
            INSERT INTO signed_state
            (channel_id, spent_balance, signature)
            VALUES (?, ?, ?)
            RETURNING *
            "#,
            channel_row.id,
            spent_balance,
            signature
        )
        .fetch_one(&self.connection)
        .await;

        match signed_state_row {
            Ok(signed_state) => Ok(signed_state),
            Err(e) => {
                error!("Error inserting signed state into database: {}", e);
                Err(ProviderError::DBError(e))
            }
        }
    }

    // Soft close a channel by setting the receiver to the closed channel account id
    pub async fn soft_close_channel(&self, channel_name: &str) -> ProviderResult<ChannelRow> {
        let _ = self.get_channel_row(channel_name).await?;
        let updated_channel_row = sqlx::query_as!(
            ChannelRow,
            r#"
            UPDATE channel
            SET soft_closed = 1
            WHERE name = ?
            RETURNING *
            "#,
            channel_name
        )
        .fetch_optional(&self.connection)
        .await;

        updated_channel_row
            .map_err(|e| {
                error!("Error soft closing channel in database: {}", e);
                ProviderError::DBError(e)
            })?
            .ok_or(ProviderError::Channel(ChannelError::NotFoundInDB))
    }

    pub async fn get_latest_signed_state(
        &self,
        channel_name: &str,
    ) -> ProviderResult<Option<SignedStateRow>> {
        info!("Getting latest signed state for channel {}", channel_name);
        let signed_state = sqlx::query_as!(
            SignedStateRow,
            r#"
                SELECT signed_state.*
                FROM signed_state
                LEFT JOIN channel ON signed_state.channel_id = channel.id
                WHERE channel.name = ?
                ORDER BY signed_state.created_at DESC
                LIMIT 1
            "#,
            channel_name,
        )
        .fetch_optional(&self.connection)
        .await;

        match signed_state {
            Ok(Some(signed_state)) => Ok(Some(signed_state)),
            Ok(None) => Ok(None),
            Err(e) => {
                error!("Error querying latest signed state from database: {}", e);
                Err(ProviderError::DBError(e))
            }
        }
    }

    pub async fn get_channel_from_signed_state(
        &self,
        signed_state: &SignedStateRow,
    ) -> ProviderResult<ChannelRow> {
        let channel = sqlx::query_as!(
            ChannelRow,
            r#"
            SELECT *
            FROM channel
            WHERE id = ?
            "#,
            signed_state.channel_id
        )
        .fetch_one(&self.connection)
        .await;

        channel.map_err(|e| {
            error!("Error querying channel from database: {}", e);
            ProviderError::DBError(e)
        })
    }

    pub async fn get_stale_channels(
        &self,
        stale_threshold: Duration,
        limit: Option<u32>,
    ) -> ProviderResult<Vec<ChannelRow>> {
        // Get all the channels that:
        // 1. are owned by the provider + are open
        // 2. haven't been updated in a while
        let updated_at_threshold = chrono::Utc::now().naive_utc() - stale_threshold;
        let limit = limit.unwrap_or(16);
        let account_id = self.account_id.to_string();
        let channels = sqlx::query_as!(
            ChannelRow,
            r#"
            SELECT *
            FROM channel
            WHERE updated_at < ? AND
                  receiver = ?
            ORDER BY updated_at DESC
            LIMIT ?
            "#,
            updated_at_threshold,
            account_id,
            limit
        )
        .fetch_all(&self.connection)
        .await;

        channels.map_err(|e| {
            error!("Error querying stale channels from database: {}", e);
            ProviderError::DBError(e)
        })
    }
}
