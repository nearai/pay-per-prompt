use std::{str::FromStr, sync::Arc, time::Duration};

use chrono::Utc;
use cli::{
    config::{SignedState, State},
    contract::{Contract as NearPaymentChannelContractClient, ContractChannel},
};
use near_crypto::Signature;
use near_sdk::{json_types::U128, AccountId, NearToken};
use serde::Serialize;
use sqlx::{sqlite::SqlitePool};
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
    pub withdraw_balance: Vec<u8>,

    pub force_close_started: Option<chrono::NaiveDateTime>,
}

impl ChannelRow {
    // A channel is in the 'default' state if it's been closed
    // so check for a 'default' value
    pub fn as_closed_result(self) -> Result<ChannelRow, ProviderError> {
        if self.receiver == CLOSED_CHANNEL_ACCOUNT_ID {
            Err(ProviderError::Channel(ChannelError::Closed))
        } else {
            Ok(self)
        }
    }

    pub fn added_balance(&self) -> NearToken {
        NearToken::from_yoctonear(u128::from_be_bytes(
            self.added_balance[..].try_into().unwrap_or([0; 16]),
        ))
    }

    pub fn withdraw_balance(&self) -> NearToken {
        NearToken::from_yoctonear(u128::from_be_bytes(
            self.withdraw_balance[..].try_into().unwrap_or([0; 16]),
        ))
    }

    pub fn is_stale(&self) -> bool {
        let now = Utc::now().naive_utc();
        let inactive_threshold = now - STALE_CHANNEL_THRESHOLD;
        self.updated_at < inactive_threshold
    }
}

impl Serialize for ChannelRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        // Convert balance Vec<u8> to u128
        let added_balance = U128::from(self.added_balance().as_yoctonear());
        let withdraw_balance = U128::from(self.withdraw_balance().as_yoctonear());

        let mut state = serializer.serialize_struct("Channel", 8)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("receiver", &self.receiver)?;
        state.serialize_field("receiver_pk", &self.receiver_pk)?;
        state.serialize_field("sender", &self.sender)?;
        state.serialize_field("sender_pk", &self.sender_pk)?;
        state.serialize_field("added_balance", &added_balance)?;
        state.serialize_field("withdraw_balance", &withdraw_balance)?;
        state.end()
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
}

impl Into<SignedState> for &SignedStateRow {
    fn into(self) -> SignedState {
        SignedState {
            state: State {
                channel_id: self.channel_id.to_string(),
                spent_balance: self.spent_balance(),
            },
            signature: Signature::from_str(&self.signature).unwrap(),
        }
    }
}

#[derive(Clone)]
pub struct ProviderDb {
    connection: SqlitePool,
    pc_client: NearPaymentChannelContractClient,
    account_id: AccountId,
}

impl ProviderDb {
    pub fn new(
        database_url: &str,
        pc_client: NearPaymentChannelContractClient,
        account_id: AccountId,
    ) -> Self {
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
            pc_client,
            account_id,
        }
    }

    pub async fn get_channel_row(&self, channel_name: &str) -> ProviderResult<ChannelRow> {
        // Query the database for the channel row
        // If the channel row is found, return it
        // If it isn't found, warn and fallback to querying the contract
        // Result wrap any non-rownotfound database errors
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
                return Ok(channel.as_closed_result()?);
            }
            Ok(None) | Err(sqlx::Error::RowNotFound) => {
                warn!("Querying channel not found in database: {}", channel_name)
            }
            Err(e) => {
                error!("Error querying channel from database: {}", e);
                return Err(ProviderError::DBError(e));
            }
        };

        // At this point, we know the channel isn't in the database, so we need to query the contract
        // as the source of truth and insert+return it
        info!(
            "Fallback to querying contract for channel: {}",
            channel_name
        );
        match self.pc_client.channel(channel_name).await {
            Some(contract_channel) => Ok(self
                .insert_channel_from_contract(channel_name, contract_channel)
                .await?),
            None => {
                info!("Channel {} not found in contract", channel_name);
                Err(ProviderError::Channel(ChannelError::NotFound))
            }
        }
    }

    pub async fn refresh_channel_row(&self, channel_name: &str) -> ProviderResult<ChannelRow> {
        match self.pc_client.channel(channel_name).await {
            Some(contract_channel) => {
                self.update_channel_from_contract(channel_name, contract_channel)
                    .await
            }
            _ => return Err(ProviderError::Channel(ChannelError::NotFound)),
        }
    }

    async fn insert_channel_from_contract(
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

        info!("Inserting channel into database: {}", channel_name);
        let contract_channel_row = sqlx::query_as!(
            ChannelRow,
            r#"
            INSERT INTO channel
            (name, sender, sender_pk, receiver, receiver_pk, added_balance, withdraw_balance)
            VALUES (?, ?, ?, ?, ?, ?, ?)
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

        match contract_channel_row {
            Ok(channel) => Ok(channel),
            Err(e) => {
                error!("Error inserting channel into database: {}", e);
                Err(ProviderError::DBError(e))
            }
        }
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
        .fetch_one(&self.connection)
        .await;

        match updated_channel_row {
            Ok(channel) => Ok(channel),
            Err(e) => {
                error!("Error updating channel in database: {}", e);
                Err(ProviderError::DBError(e))
            }
        }
    }

    async fn update_channel_from_contract(
        &self,
        channel_name: &str,
        contract_channel: ContractChannel,
    ) -> ProviderResult<ChannelRow> {
        let channel_row = self.get_channel_row(channel_name).await?;
        let sender = contract_channel.sender.account_id.to_string();
        let receiver = contract_channel.receiver.account_id.to_string();
        let updated_added_balance = contract_channel
            .added_balance
            .as_yoctonear()
            .to_be_bytes()
            .to_vec();
        let updated_withdrawn_balance = contract_channel
            .withdrawn_balance
            .as_yoctonear()
            .to_be_bytes()
            .to_vec();
        let force_close_started: Option<chrono::DateTime<chrono::Utc>> = contract_channel
            .force_close_started
            .map(|v| sqlx::types::chrono::DateTime::from_timestamp_nanos(v as i64));
        info!("Updating channel {} in database", channel_name);
        let updated_channel_row = sqlx::query_as!(
            ChannelRow,
            r#"
            UPDATE channel
            SET updated_at = CURRENT_TIMESTAMP,
                sender = ?,
                receiver = ?,
                added_balance = ?,
                withdraw_balance = ?,
                force_close_started = ?
            WHERE id = ?
            RETURNING *
            "#,
            sender,
            receiver,
            updated_added_balance,
            updated_withdrawn_balance,
            force_close_started,
            channel_row.id
        )
        .fetch_one(&self.connection)
        .await;

        match updated_channel_row {
            Ok(channel) => Ok(channel),
            Err(e) => {
                error!("Error updating channel in database: {}", e);
                Err(ProviderError::DBError(e))
            }
        }
    }

    pub async fn insert_signed_state(
        &self,
        signed_state: &SignedState,
    ) -> ProviderResult<SignedStateRow> {
        let channel_row = self.get_channel_row(&signed_state.state.channel_id).await?;

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

    pub async fn latest_signed_state(
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

    pub async fn get_stale_channels(
        &self,
        stale_threshold: Duration,
        limit: Option<u32>,
    ) -> ProviderResult<Vec<ChannelRow>> {
        // Get all the channels that:
        // 1. haven't been closed due to inactivity
        // 2. have been updated in a while
        // 3. are owned by the provider
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

        match channels {
            Ok(channels) => Ok(channels),
            Err(e) => {
                error!("Error querying stale channels from database: {}", e);
                Err(ProviderError::DBError(e))
            }
        }
    }
}
