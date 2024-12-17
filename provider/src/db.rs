use cli::contract::{Contract as NearPaymentChannelContractClient, ContractChannel};
use near_primitives::types::AccountId;
use near_sdk::json_types::U128;
use serde::Serialize;
use sqlx::sqlite::SqlitePool;
use tracing::info;

#[derive(Default, Debug, sqlx::FromRow)]
pub struct ChannelRow {
    pub id: i64,
    pub name: String,
    pub receiver: String,
    pub receiver_pk: String,
    pub sender: String,
    pub sender_pk: String,
    // Near Tokens in contracts are represented as u128's
    // this isn't supported by sqlite, so we store them as bytes
    pub added_balance: Vec<u8>,
    pub withdraw_balance: Vec<u8>,
}

impl Serialize for ChannelRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        // Convert balance Vec<u8> to u128
        let added_balance = U128::from(u128::from_be_bytes(
            self.added_balance[..].try_into().unwrap_or([0; 16]),
        ));
        let withdraw_balance = U128::from(u128::from_be_bytes(
            self.withdraw_balance[..].try_into().unwrap_or([0; 16]),
        ));

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

#[derive(Clone)]
pub struct ProviderDb {
    connection: SqlitePool,
    pc_client: NearPaymentChannelContractClient,
}

impl ProviderDb {
    pub fn new(database_url: &str, pc_client: NearPaymentChannelContractClient) -> Self {
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
        }
    }

    async fn get_channel_row(&self, channel_name: &str) -> Result<Option<ChannelRow>, sqlx::Error> {
        let channel = sqlx::query_as!(
            ChannelRow,
            "SELECT * FROM channel WHERE name = ? LIMIT 1",
            channel_name
        )
        .fetch_optional(&self.connection)
        .await?;

        Ok(channel)
    }

    async fn insert_channel_from_contract(
        &self,
        channel_name: &str,
        contract_channel: ContractChannel,
    ) -> Result<ChannelRow, sqlx::Error> {
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
        .await?;

        Ok(contract_channel_row)
    }

    async fn update_channel_from_contract(
        &self,
        channel_name: &str,
        contract_channel: ContractChannel,
    ) -> Result<Option<ChannelRow>, sqlx::Error> {
        let channel_row = self.get_channel_row(channel_name).await?;
        match channel_row {
            None => Err(sqlx::Error::RowNotFound),
            Some(channel_row) => {
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
                let updated_channel_row = sqlx::query_as!(
                    ChannelRow,
                    r#"
                    UPDATE channel
                    SET added_balance = ?, withdraw_balance = ?
                    WHERE id = ?
                    RETURNING *
                    "#,
                    updated_added_balance,
                    updated_withdrawn_balance,
                    channel_row.id
                )
                .fetch_one(&self.connection)
                .await?;

                Ok(Some(updated_channel_row))
            }
        }
    }

    pub async fn refresh_channel_row(
        &self,
        channel_name: &str,
    ) -> Result<Option<ChannelRow>, sqlx::Error> {
        let channel_row = self.get_channel_row(channel_name).await?;
        let contract_channel = self.pc_client.channel(channel_name).await;
        match (channel_row, contract_channel) {
            (Some(_), Some(contract_channel)) => {
                self.update_channel_from_contract(channel_name, contract_channel)
                    .await
            }
            (None, Some(contract_channel)) => Ok(Some(
                self.insert_channel_from_contract(channel_name, contract_channel)
                    .await?,
            )),
            _ => return Err(sqlx::Error::RowNotFound),
        }
    }

    pub async fn get_channel_row_or_refresh(
        &self,
        channel_name: &str,
    ) -> Result<Option<ChannelRow>, sqlx::Error> {
        // Check if the channel exists in the db, if it does, return it
        let channel = self.get_channel_row(channel_name).await?;
        if channel.is_some() {
            return Ok(channel);
        }

        // If we aren't tracking the channel, query the contract to get the channel state, save it, and return it
        let contract_channel = self.pc_client.channel(channel_name).await;
        match contract_channel {
            Some(contract_channel) => Ok(Some(
                self.insert_channel_from_contract(channel_name, contract_channel)
                    .await?,
            )),
            None => Err(sqlx::Error::RowNotFound),
        }
    }

    pub async fn latest_signed_state(
        &self,
        channel_name: &str,
    ) -> Result<Option<SignedStateRow>, sqlx::Error> {
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
        .await?;
        Ok(signed_state)
    }
}
