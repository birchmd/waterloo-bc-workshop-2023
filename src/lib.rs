use near_sdk::{borsh::{self, BorshDeserialize, BorshSerialize}, near_bindgen};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct MessengerContract;

#[near_bindgen]
impl MessengerContract {
    pub fn hello_world(&self) -> String {
        "Hello, World!".into()
    }
}
