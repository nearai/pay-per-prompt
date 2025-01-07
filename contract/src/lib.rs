use fraction::Fraction;
use near_sdk::borsh::to_vec;
use near_sdk::store::{LazyOption, LookupMap};
use near_sdk::{
    env, near, near_bindgen, require, AccountId, NearToken, PanicOnDefault, Promise, PublicKey,
    Timestamp,
};
use signature::Signature;
use std::str::FromStr;

mod fraction;
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

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Ownership {
    owner: AccountId,
    fee: Fraction,
    balance: NearToken,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    channels: LookupMap<ChannelId, Channel>,
    ownership: LazyOption<Ownership>,
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
        assert!(pk_raw[0] == 0, "Invalid public key");
        let pk_raw_32: [u8; 32] = pk_raw[1..].try_into().unwrap();
        env::ed25519_verify(self.signature.as_ref(), message.as_ref(), &pk_raw_32)
    }
}

#[near_bindgen]
impl Contract {
    #[init]
    #[private]
    pub fn init() -> Contract {
        Contract {
            channels: LookupMap::new(b"c".to_vec()),
            ownership: LazyOption::new(b"o", None),
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

        let after_fee = self.owner_collect_fee(difference);

        Promise::new(receiver).transfer(after_fee)
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

    pub fn withdraw_and_close(&mut self, state: SignedState, close: SignedState) -> Promise {
        self.withdraw(state).then(self.close(close))
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
}

// Owner methods
#[near_bindgen]
impl Contract {
    /// Show owner information publicly
    pub fn owner(&self) -> Option<Ownership> {
        self.ownership.get().clone()
    }

    #[private]
    pub fn owner_update(&mut self, new_owner: Option<AccountId>, new_fee: Fraction) {
        require!(new_fee.is_less_than_one(), "Fee must be less than 1");

        let owner_balance = self
            .ownership
            .get()
            .as_ref()
            .map(|o| o.balance)
            .unwrap_or_default();

        if new_owner.is_none() {
            require!(
                new_fee.is_zero(),
                "New fee must be zero when removing the owner"
            );
            require!(
                owner_balance.as_yoctonear() == 0,
                "Owner balance must be zero when removing the owner"
            );

            self.ownership.set(None);
        } else {
            self.ownership.set(Some(Ownership {
                owner: new_owner.unwrap(),
                fee: new_fee,
                balance: owner_balance,
            }));
        }
    }

    pub fn owner_withdraw(&mut self) -> Promise {
        let Ownership {
            owner,
            fee,
            balance,
        } = self.ownership.get().clone().unwrap();

        require!(balance.as_yoctonear() > 0, "No balance to withdraw");

        self.ownership.set(Some(Ownership {
            owner: owner.clone(),
            fee,
            balance: NearToken::from_yoctonear(0),
        }));

        Promise::new(owner).transfer(balance)
    }

    fn owner_collect_fee(&mut self, amount: NearToken) -> NearToken {
        if let Some(ownership) = self.ownership.get_mut() {
            let fee_amount = ownership.fee.mul_balance(amount);
            let remaining_amount = amount.saturating_sub(fee_amount);
            ownership.balance = ownership.balance.saturating_add(fee_amount);
            remaining_amount
        } else {
            // No owner, no fee to collect
            amount
        }
    }
}

// Migration methods
#[near_bindgen]
impl Contract {
    #[private]
    #[init(ignore_state)]
    pub fn migrate() -> Self {
        #[derive(borsh::BorshDeserialize)]
        struct OldContract {
            channels: LookupMap<ChannelId, Channel>,
        }

        let contract = env::state_read::<OldContract>().unwrap();

        Self {
            channels: contract.channels,
            ownership: LazyOption::new(b"o", None),
        }
    }
}
