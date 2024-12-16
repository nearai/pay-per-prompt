use near_sdk::borsh::to_vec;
use near_sdk::store::LookupMap;
use near_sdk::{
    env, near, near_bindgen, require, AccountId, NearToken, PanicOnDefault, Promise, PublicKey,
    Timestamp,
};
use signature::Signature;
use std::str::FromStr;

mod signature;

type ChannelId = String;

const SECOND: u64 = 1_000_000_000;
const DAY: u64 = 24 * 60 * 60 * SECOND;
const HARD_CLOSE_TIMEOUT: u64 = 7 * DAY;

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Account {
    account_id: AccountId,
    public_key: PublicKey,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            account_id: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string()
                .try_into()
                .unwrap(),
            public_key: PublicKey::from_str("ed25519:11111111111111111111111111111111").unwrap(),
        }
    }
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Default)]
pub struct Channel {
    receiver: Account,
    sender: Account,
    added_balance: NearToken,
    withdrawn_balance: NearToken,
    force_close_started: Option<Timestamp>,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    channels: LookupMap<ChannelId, Channel>,
}

#[near(serializers = [borsh, json])]
struct State {
    channel_id: ChannelId,
    spent_balance: NearToken,
}

#[near(serializers = [borsh, json])]
pub struct SignedState {
    state: State,
    signature: Signature,
}

impl SignedState {
    fn verify(&self, pk: &PublicKey) -> bool {
        let message = to_vec(&self.state).unwrap();
        let pk_raw = pk.as_bytes();

        env::ed25519_verify(
            self.signature.as_ref(),
            message.as_ref(),
            pk_raw.try_into().unwrap(),
        )
    }
}

#[near_bindgen]
impl Contract {
    #[init]
    #[private]
    pub fn init() -> Contract {
        Contract {
            channels: LookupMap::new(b"c".to_vec()),
        }
    }

    #[payable]
    pub fn open_channel(&mut self, channel_id: ChannelId, receiver: Account, sender: Account) {
        require!(
            !self.channels.contains_key(&channel_id),
            "Channel already exists"
        );

        let channel = Channel {
            receiver,
            sender,
            added_balance: env::attached_deposit(),
            withdrawn_balance: NearToken::from_yoctonear(0),
            force_close_started: None,
        };

        self.channels.insert(channel_id, channel);
    }

    pub fn withdraw(&mut self, state: SignedState) -> Promise {
        let channel_id = state.state.channel_id.clone();

        let channel = self.channels.get_mut(&channel_id).unwrap();

        require!(
            env::predecessor_account_id() == channel.receiver.account_id,
            "Only receiver can withdraw"
        );

        require!(
            state.verify(&channel.sender.public_key),
            "Invalid signature from sender"
        );

        require!(
            channel.withdrawn_balance < state.state.spent_balance,
            "No balance to withdraw"
        );

        let difference = state
            .state
            .spent_balance
            .saturating_sub(channel.withdrawn_balance);

        let receiver = channel.receiver.account_id.clone();

        channel.withdrawn_balance = state.state.spent_balance;

        Promise::new(receiver).transfer(difference)
    }

    #[payable]
    pub fn topup(&mut self, channel_id: ChannelId) {
        let channel = self.channels.get_mut(&channel_id).unwrap();
        require!(channel.force_close_started.is_none(), "Channel is closing.");
        let amount = env::attached_deposit();
        channel.added_balance = channel.added_balance.saturating_add(amount);
    }

    pub fn close(&mut self, state: SignedState) -> Promise {
        let channel_id = state.state.channel_id.clone();

        let channel = self.channels.get_mut(&channel_id).unwrap();

        // Anyone can close the channel, as long as it has a signature from the receiver
        require!(
            state.verify(&channel.receiver.public_key),
            "Invalid signature from receiver"
        );

        require!(
            state.state.spent_balance.as_yoctonear() == 0,
            "Invalid payload",
        );

        let remaining_balance = channel
            .added_balance
            .saturating_sub(channel.withdrawn_balance);

        let sender = channel.sender.account_id.clone();

        // Remove channel from the state
        //
        // This is equivalent to remove the channel, though we keep it in the state
        // so no new channel with the same id is created in the future. If the same
        // channel is reused (either provider or user could trick each other) by
        // reusing an old channel id and replaying old messages.
        self.channels.insert(channel_id, Default::default());

        Promise::new(sender).transfer(remaining_balance)
    }

    pub fn force_close_start(&mut self, channel_id: ChannelId) {
        let channel = self.channels.get_mut(&channel_id).unwrap();

        require!(
            channel.force_close_started.is_none(),
            "Channel is already closing."
        );

        require!(
            env::predecessor_account_id() == channel.sender.account_id,
            "Only sender can start a force close action"
        );

        channel.force_close_started = Some(env::block_timestamp());
    }

    pub fn force_close_finish(&mut self, channel_id: ChannelId) -> Promise {
        let channel = self.channels.get_mut(&channel_id).unwrap();

        match channel.force_close_started {
            Some(start_event) => {
                let difference = env::block_timestamp() - start_event;
                if difference >= HARD_CLOSE_TIMEOUT {
                    let remaining_balance = channel
                        .added_balance
                        .saturating_sub(channel.withdrawn_balance);

                    let sender = channel.sender.account_id.clone();

                    // Remove channel from the state [See message above]
                    self.channels.insert(channel_id, Default::default());

                    Promise::new(sender).transfer(remaining_balance)
                } else {
                    env::panic_str("Channel can't be closed yet. Not enough time has passed.");
                }
            }
            None => {
                env::panic_str("Channel is not closing.");
            }
        }
    }

    pub fn channel(&self, channel_id: ChannelId) -> Option<Channel> {
        self.channels.get(&channel_id).cloned()
    }

    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        Self::init()
    }
}
