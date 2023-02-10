use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    env,
    json_types::U64,
    serde::{Deserialize, Serialize},
    AccountId,
};

/// Different possible responses when we attempt to add an account as a contact.
#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum AddContactResponse {
    /// This account does not support the Messenger protocol.
    InvalidAccount,
    /// This account as blocked us from sending requests.
    Blocked,
    /// The request was ignored because we are already contacts.
    AlreadyConnected,
    /// The request did not come with a sufficient deposit.
    InsufficientDeposit,
    /// The request was accepted and is pending a response.
    Pending,
    /// The request was accepted and we are now contacts of one another.
    Accepted,
}

/// Different possible responses when we accept an add contact request.
#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum AcceptContactResponse {
    /// This account does not support the Messenger protocol.
    InvalidAccount,
    /// There was no pending request from the account.
    UnknownAccount,
    /// The account blocked us, so we can not longer accept their request.
    Blocked,
    /// The acceptance was ignored because we are already contacts.
    AlreadyConnected,
    /// The contact was successfully added.
    Accepted,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum MessageResponse {
    InvalidAccount,
    Blocked,
    NotConnected,
    InsufficientDeposit,
    Received,
}

/// Unique ID for messages the contract receives.
#[derive(Debug, Copy, Clone, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct MessageId(pub near_sdk::CryptoHash);

#[derive(Debug, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Message {
    pub sender: AccountId,
    pub timestamp: U64,
    pub content: String,
    pub parent_id: Option<MessageId>,
}

impl Message {
    pub fn id(&self) -> MessageId {
        let bytes = self
            .try_to_vec()
            .unwrap_or_else(|_e| env::panic_str("Failed to serialize message"));
        let hash = env::sha256_array(&bytes);
        MessageId(hash)
    }
}

#[derive(Debug, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum MessageStatus {
    Read,
    Unread,
}

/// The status of another account from the perspective of our contract.
#[derive(Debug, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum AccountStatus {
    /// No record of the account.
    Unknown,
    /// We sent a request to connect with the account, but no reply.
    SentPendingRequest,
    /// We received a request to connect with the account, but have not accepted it yet.
    ReceivedPendingRequest,
    /// We have blocked interactions with that account.
    Blocked,
    /// Known account that we can interact with.
    Contact,
}
