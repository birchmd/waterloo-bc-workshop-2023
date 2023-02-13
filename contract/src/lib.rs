use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, UnorderedSet},
    env,
    json_types::U64,
    near_bindgen, require, AccountId, Balance, BorshStorageKey, PanicOnDefault, Promise,
    PromiseError, PromiseOrValue,
};
use types::{
    AcceptContactResponse, AccountStatus, AddContactResponse, Message, MessageId, MessageResponse,
    MessageStatus, MessageWithId, UnreadMessageView,
};

pub mod types;

/// A deposit is required to send a contact request. This is meant to discourage spam and
/// to cover the cost of inserting a storage key into another contract.
/// Note: 1 Near = 10^24 yoctoNear (the units of the Balance type).
const ADD_CONTACT_DEPOSIT: Balance = env::STORAGE_PRICE_PER_BYTE;

/// Number of messages shown in a view call by default.
const DEFAULT_THREAD_SIZE: usize = 8;

/// Enum to different different sections of the contract storage.
#[derive(BorshDeserialize, BorshSerialize, BorshStorageKey)]
pub enum StoragePrefix {
    Accounts,
    Messages,
    MessageStatuses(MessageStatus),
    LastReceivedMessage,
    PendingContacts,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MessengerContract {
    accounts: LookupMap<AccountId, AccountStatus>,
    messages: LookupMap<MessageId, Message>,
    unread_messages: UnorderedSet<MessageId>,
    read_messages: UnorderedSet<MessageId>,
    last_received_message: LookupMap<AccountId, MessageId>,
    pending_contacts: UnorderedSet<AccountId>,
    owner: AccountId,
}

#[near_bindgen]
impl MessengerContract {
    #[init]
    pub fn new() -> Self {
        Self {
            accounts: LookupMap::new(StoragePrefix::Accounts),
            messages: LookupMap::new(StoragePrefix::Messages),
            unread_messages: UnorderedSet::new(StoragePrefix::MessageStatuses(
                MessageStatus::Unread,
            )),
            read_messages: UnorderedSet::new(StoragePrefix::MessageStatuses(MessageStatus::Read)),
            last_received_message: LookupMap::new(StoragePrefix::LastReceivedMessage),
            pending_contacts: UnorderedSet::new(StoragePrefix::PendingContacts),
            owner: env::predecessor_account_id(),
        }
    }

    /// View a single message.
    /// Note: this function does not mutate the contract. Therefore it can be done as a
    /// view call, but also will not mark unread messages as read.
    pub fn view_message(&self, message_id: MessageId) -> Option<Message> {
        self.messages.get(&message_id)
    }

    pub fn view_unread(&self, max_size: Option<usize>) -> Vec<UnreadMessageView> {
        let unread_set = &self.unread_messages;
        let num_messages = unread_set.len() as usize;
        let num_to_view = max_size.unwrap_or(num_messages).min(num_messages);
        let mut result = Vec::with_capacity(num_to_view);
        for id in unread_set.iter().take(num_to_view) {
            let message = self.get_message(&id);
            let view = UnreadMessageView {
                id,
                sender: message.sender,
                timestamp: message.timestamp,
            };
            result.push(view);
        }
        result
    }

    /// Shows the history of messages we have received from the given `sender`.
    pub fn view_thread(&self, sender: AccountId, max_size: Option<usize>) -> Vec<MessageWithId> {
        let max_size = max_size.unwrap_or(DEFAULT_THREAD_SIZE);
        let last_message = match self.last_received_message.get(&sender) {
            Some(id) => id,
            None => return Vec::new(),
        };
        let mut result = Vec::with_capacity(max_size);
        let mut current_message = MessageWithId {
            id: last_message,
            message: self.get_message(&last_message),
        };
        for _ in 0..max_size {
            let next_message = current_message.message.parent_id;
            result.push(current_message);
            match next_message {
                Some(id) => {
                    current_message = MessageWithId {
                        id,
                        message: self.get_message(&id),
                    };
                }
                None => break,
            }
        }
        // We read the thread from most recent to least, so we reverse the order
        // for the benefit of the user.
        result.reverse();
        result
    }

    pub fn view_pending_contacts(&self, max_size: Option<usize>) -> Vec<AccountId> {
        match max_size {
            Some(size) => self.pending_contacts.iter().take(size).collect(),
            None => self.pending_contacts.iter().collect(),
        }
    }

    /// In contrast to `view_message`, this function actually marks the message as read.
    /// Therefore, this must be done as a real transaction, not just a view call.
    pub fn read_message(&mut self, message_id: MessageId) -> Option<Message> {
        self.require_owner_only();

        let was_unread = self.unread_messages.remove(&message_id);
        if was_unread {
            self.read_messages.insert(&message_id);
        }
        self.messages.get(&message_id)
    }

    /// Send a message to one of your contacts.
    #[payable]
    pub fn send_message(&mut self, account: AccountId, message: String) -> Promise {
        self.require_owner_only();

        let required_deposit = compute_required_message_deposit(&message);
        let deposit = env::attached_deposit();
        require!(deposit >= required_deposit, "Insufficient deposit");

        require!(
            matches!(self.accounts.get(&account), Some(AccountStatus::Contact)),
            "You can only send messages to your contacts!"
        );

        Self::ext(account)
            .with_attached_deposit(deposit)
            .receive_message(message)
    }

    /// Called by another Messenger contract when their user wants to send us a message.
    #[payable]
    pub fn receive_message(&mut self, content: String) -> MessageResponse {
        let required_deposit = compute_required_message_deposit(&content);
        let deposit = env::attached_deposit();
        if deposit < required_deposit {
            return MessageResponse::InsufficientDeposit;
        }

        let sender = env::predecessor_account_id();
        let status = self.accounts.get(&sender).unwrap_or(AccountStatus::Unknown);
        match status {
            AccountStatus::Contact => {
                let parent_id = self.last_received_message.get(&sender);
                let timestamp = env::block_timestamp();
                let message = Message {
                    content,
                    sender: sender.clone(),
                    parent_id,
                    timestamp: U64(timestamp),
                };
                let message_id = message.id();
                self.messages.insert(&message_id, &message);
                self.unread_messages.insert(&message_id);
                self.last_received_message.insert(&sender, &message_id);

                MessageResponse::Received
            }
            AccountStatus::Blocked => MessageResponse::Blocked,
            AccountStatus::Unknown
            | AccountStatus::ReceivedPendingRequest
            | AccountStatus::SentPendingRequest => MessageResponse::NotConnected,
        }
    }

    /// `add_contact` flow:
    /// 1. Call `ext_add_contact` in the account we wish to add as a contact.
    ///    This ensures the account understands the Messenger protocol and that they
    ///    haven't already blocked us.
    /// 2. Check the response from the account in a callback.
    #[payable]
    pub fn add_contact(&mut self, account: AccountId) -> Promise {
        self.require_owner_only();

        let deposit = env::attached_deposit();
        require!(deposit >= ADD_CONTACT_DEPOSIT, "Insufficient deposit");

        let this = env::current_account_id();
        Self::ext(account.clone())
            .with_attached_deposit(deposit)
            .ext_add_contact()
            .then(Self::ext(this).add_contact_callback(account))
    }

    /// Part of the `add_contact` flow. This method is called by another Messenger contract
    /// when it wants to add us as a contact. If we don't know this account then we add
    /// that we have received a pending request (which we may choose to accept).
    #[payable]
    pub fn ext_add_contact(&mut self) -> AddContactResponse {
        let deposit = env::attached_deposit();
        if deposit < ADD_CONTACT_DEPOSIT {
            return AddContactResponse::InsufficientDeposit;
        }

        let request_sender = env::predecessor_account_id();
        let current_status = self
            .accounts
            .get(&request_sender)
            .unwrap_or(AccountStatus::Unknown);
        match current_status {
            AccountStatus::Unknown => {
                self.accounts
                    .insert(&request_sender, &AccountStatus::ReceivedPendingRequest);
                self.pending_contacts.insert(&request_sender);
                AddContactResponse::Pending
            }
            AccountStatus::SentPendingRequest => {
                // We had sent a contact request and they added us back, so let's accept
                self.accounts
                    .insert(&request_sender, &AccountStatus::Contact);
                self.pending_contacts.remove(&request_sender);
                AddContactResponse::Accepted
            }
            AccountStatus::ReceivedPendingRequest => AddContactResponse::Pending,
            AccountStatus::Blocked => AddContactResponse::Blocked,
            AccountStatus::Contact => AddContactResponse::AlreadyConnected,
        }
    }

    /// `accept_contact` flow:
    /// 1. Pre-requisite: the target account sent us a contact request via `add_contact`.
    /// 2. Call `ext_accept_contact` in the other account, to communicate the request is accepted.
    /// 3. Check the response from the account in a callback.
    pub fn accept_contact(&mut self, account: AccountId) -> PromiseOrValue<AcceptContactResponse> {
        self.require_owner_only();

        let current_status = self
            .accounts
            .get(&account)
            .unwrap_or(AccountStatus::Unknown);
        match current_status {
            AccountStatus::ReceivedPendingRequest => {
                let this = env::current_account_id();
                Self::ext(account.clone())
                    .ext_accept_contact()
                    .then(Self::ext(this).accept_contact_callback(account))
                    .into()
            }
            AccountStatus::Contact => {
                PromiseOrValue::Value(AcceptContactResponse::AlreadyConnected)
            }
            AccountStatus::Blocked | AccountStatus::SentPendingRequest | AccountStatus::Unknown => {
                PromiseOrValue::Value(AcceptContactResponse::UnknownAccount)
            }
        }
    }

    /// Part of the `accept_contact` flow. This method is called by another Messenger contract
    /// to accept our request to become contacts. If we had sent a request then we mark them
    /// as a contact.
    pub fn ext_accept_contact(&mut self) -> AcceptContactResponse {
        let sender = env::predecessor_account_id();
        let current_status = self.accounts.get(&sender).unwrap_or(AccountStatus::Unknown);
        match current_status {
            AccountStatus::SentPendingRequest => {
                self.accounts.insert(&sender, &AccountStatus::Contact);
                self.pending_contacts.remove(&sender);
                AcceptContactResponse::Accepted
            }
            AccountStatus::Blocked => AcceptContactResponse::Blocked,
            AccountStatus::Contact => AcceptContactResponse::AlreadyConnected,
            AccountStatus::ReceivedPendingRequest | AccountStatus::Unknown => {
                AcceptContactResponse::UnknownAccount
            }
        }
    }

    #[private]
    pub fn add_contact_callback(
        &mut self,
        account: AccountId,
        #[callback_result] response: Result<AddContactResponse, PromiseError>,
    ) -> AddContactResponse {
        match response {
            Ok(AddContactResponse::Pending) => {
                self.accounts
                    .insert(&account, &AccountStatus::SentPendingRequest);
                AddContactResponse::Pending
            }
            Ok(AddContactResponse::Accepted) => {
                self.accounts.insert(&account, &AccountStatus::Contact);
                AddContactResponse::Accepted
            }
            Ok(AddContactResponse::AlreadyConnected) => {
                let previous_status = self.accounts.insert(&account, &AccountStatus::Contact);
                if let Some(AccountStatus::Contact) = previous_status {
                    AddContactResponse::AlreadyConnected
                } else {
                    AddContactResponse::Accepted
                }
            }
            Ok(other_response) => other_response,
            Err(_e) => AddContactResponse::InvalidAccount,
        }
    }

    #[private]
    pub fn accept_contact_callback(
        &mut self,
        account: AccountId,
        #[callback_result] response: Result<AcceptContactResponse, PromiseError>,
    ) -> AcceptContactResponse {
        match response {
            Ok(AcceptContactResponse::Accepted) => {
                self.accounts.insert(&account, &AccountStatus::Contact);
                self.pending_contacts.remove(&account);
                AcceptContactResponse::Accepted
            }
            Ok(other_response) => other_response,
            Err(_e) => AcceptContactResponse::InvalidAccount,
        }
    }
}

impl MessengerContract {
    fn require_owner_only(&self) {
        require!(
            self.owner == env::predecessor_account_id(),
            "Only the owner can use this method!"
        );
    }

    fn get_message(&self, id: &MessageId) -> Message {
        self.messages
            .get(id)
            .unwrap_or_else(|| env::panic_str("Missing message"))
    }
}

fn compute_required_message_deposit(message: &str) -> Balance {
    (message.len() as Balance) * env::STORAGE_PRICE_PER_BYTE
}
