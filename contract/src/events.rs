//! Follows the Near standard for eventing.
//! See https://github.com/near/NEPs/blob/master/specs/Standards/EventsFormat.md

use crate::types;
use near_sdk::{
    env,
    serde::{Deserialize, Serialize},
    serde_json, AccountId,
};
use std::borrow::Cow;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct Event<'a> {
    pub standard: Cow<'static, str>,
    pub version: Cow<'static, str>,
    #[serde(flatten)]
    pub event_kind: EventKind<'a>,
}

impl<'a> Event<'a> {
    /// This should be a NEP number (after making a NEP proposal of course), but this is just an
    /// example, not a real Near standard.
    pub const STANDARD: &str = "NearMessenger";
    pub const VERSION: &str = "1.0.0";

    /// Create an event for having sent a pending contact request.
    pub fn pending_contact_request(sender: &'a AccountId, receiver: &'a AccountId) -> Self {
        let kind = EventKind::PendingContactRequest(PendingContactRequest {
            sender: sender.borrowed(),
            receiver: receiver.borrowed(),
        });
        Self::with_kind(kind)
    }

    /// Create an event for having received a pending contact request.
    pub fn received_contact_request(sender: &'a AccountId, receiver: &'a AccountId) -> Self {
        let kind = EventKind::ReceivedContactRequest(PendingContactRequest {
            sender: sender.borrowed(),
            receiver: receiver.borrowed(),
        });
        Self::with_kind(kind)
    }

    /// Create an event for having added a new contact.
    /// `this` refers to the account that has added the new contact.
    pub fn new_contact(this: &'a AccountId, contact: &'a AccountId) -> Self {
        let kind = EventKind::NewContact(NewContact {
            this: this.borrowed(),
            contact: contact.borrowed(),
        });
        Self::with_kind(kind)
    }

    /// Create an event for having sent a message.
    pub fn message_sent(sender: &'a AccountId, receiver: &'a AccountId) -> Self {
        let kind = EventKind::MessageSent(MessageSent {
            sender: sender.borrowed(),
            receiver: receiver.borrowed(),
        });
        Self::with_kind(kind)
    }

    /// Create an event for having received a message.
    pub fn message_received(
        sender: &'a AccountId,
        receiver: &'a AccountId,
        id: &'a types::MessageId,
    ) -> Self {
        let kind = EventKind::MessageReceived(MessageReceived {
            sender: sender.borrowed(),
            receiver: receiver.borrowed(),
            message_id: id.borrowed(),
        });
        Self::with_kind(kind)
    }

    /// Must call this method to actually emit the event into the Near logs.
    pub fn emit(self) {
        env::log_str(&self.to_log());
    }

    /// Turn this event into a String formatted as it would be in the Near logs.
    pub fn to_log(&self) -> String {
        format!(
            "EVENT_JSON:{}",
            serde_json::to_string(&self).unwrap_or_default()
        )
    }

    pub fn as_pending_contact_request(&self) -> Option<&PendingContactRequest<'a>> {
        match &self.event_kind {
            EventKind::PendingContactRequest(x) => Some(x),
            EventKind::ReceivedContactRequest(x) => Some(x),
            _ => None,
        }
    }

    pub fn as_new_contact(&self) -> Option<&NewContact<'a>> {
        match &self.event_kind {
            EventKind::NewContact(x) => Some(x),
            _ => None,
        }
    }

    pub fn as_message_sent(&self) -> Option<&MessageSent<'a>> {
        match &self.event_kind {
            EventKind::MessageSent(x) => Some(x),
            _ => None,
        }
    }

    pub fn as_message_received(&self) -> Option<&MessageReceived<'a>> {
        match &self.event_kind {
            EventKind::MessageReceived(x) => Some(x),
            _ => None,
        }
    }

    fn with_kind(event_kind: EventKind<'a>) -> Self {
        Self {
            standard: Cow::Borrowed(Self::STANDARD),
            version: Cow::Borrowed(Self::VERSION),
            event_kind,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
#[serde(tag = "event", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum EventKind<'a> {
    PendingContactRequest(PendingContactRequest<'a>),
    ReceivedContactRequest(PendingContactRequest<'a>),
    NewContact(NewContact<'a>),
    MessageSent(MessageSent<'a>),
    MessageReceived(MessageReceived<'a>),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct PendingContactRequest<'a> {
    pub sender: Cow<'a, AccountId>,
    pub receiver: Cow<'a, AccountId>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct NewContact<'a> {
    pub this: Cow<'a, AccountId>,
    pub contact: Cow<'a, AccountId>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct MessageSent<'a> {
    pub sender: Cow<'a, AccountId>,
    pub receiver: Cow<'a, AccountId>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(crate = "near_sdk::serde")]
pub struct MessageReceived<'a> {
    pub sender: Cow<'a, AccountId>,
    pub receiver: Cow<'a, AccountId>,
    pub message_id: Cow<'a, types::MessageId>,
}

// Helper trait to enabled the `.borrowed` syntax above
trait AsBorrowed<'a, T: Clone> {
    fn borrowed(self) -> Cow<'a, T>;
}

impl<'a, T: Clone> AsBorrowed<'a, T> for &'a T {
    fn borrowed(self) -> Cow<'a, T> {
        Cow::Borrowed(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_format() {
        let sender: AccountId = "alice.near".parse().unwrap();
        let receiver: AccountId = "bob.near".parse().unwrap();
        let event = Event::pending_contact_request(&sender, &receiver);
        let log_output = event.to_log();
        assert_eq!(
            log_output,
            format!(
                r#"EVENT_JSON:{{"standard":"{}","version":"{}","event":"pending_contact_request","data":{{"sender":"{}","receiver":"{}"}}}}"#,
                Event::STANDARD,
                Event::VERSION,
                sender.as_str(),
                receiver.as_str()
            ),
        );
    }
}
