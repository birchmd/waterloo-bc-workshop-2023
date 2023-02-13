#[cfg(test)]
mod tests {
    use aurora_sdk_integration_tests::{
        tokio, utils,
        workspaces::{self, AccountId},
    };
    use near_messenger::types;

    #[tokio::test]
    async fn test_near_messenger() {
        let worker = workspaces::sandbox().await.unwrap();

        let alice = setup_messenger_contract("alice.test.near", &worker).await;
        let bob = setup_messenger_contract("bob.test.near", &worker).await;

        // Alice adds Bob as a contact
        let response: types::AddContactResponse = alice
            .owner
            .call(alice.contract.id(), "add_contact")
            .args_json(serde_json::json!({
                "account": "chat.bob.test.near",
            }))
            .deposit(1_000_000_000_000_000_000_000_000)
            .max_gas()
            .transact()
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(response, types::AddContactResponse::Pending);

        let pending_contacts: Vec<AccountId> = bob
            .owner
            .view(bob.contract.id(), "view_pending_contacts")
            .args(b"{}".to_vec())
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(pending_contacts.len(), 1);

        // Bob accepts Alice as a contact
        let response: types::AcceptContactResponse = bob
            .owner
            .call(bob.contract.id(), "accept_contact")
            .args_json(serde_json::json!({
                "account": "chat.alice.test.near",
            }))
            .max_gas()
            .transact()
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(response, types::AcceptContactResponse::Accepted);

        // No longer any pending requests after Bob accepts
        let pending_contacts: Vec<AccountId> = bob
            .owner
            .view(bob.contract.id(), "view_pending_contacts")
            .args(b"{}".to_vec())
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(pending_contacts.len(), 0);

        // Alice sends Bob a message
        let response: types::MessageResponse = alice
            .owner
            .call(alice.contract.id(), "send_message")
            .args_json(serde_json::json!({
                "account": "chat.bob.test.near",
                "message": "Hello, Bob!",
            }))
            .deposit(1_000_000_000_000_000_000_000_000)
            .max_gas()
            .transact()
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(response, types::MessageResponse::Received);

        let unread: Vec<types::UnreadMessageView> = bob
            .owner
            .view(bob.contract.id(), "view_unread")
            .args(b"{}".to_vec())
            .await
            .unwrap()
            .json()
            .unwrap();
        assert_eq!(unread.len(), 1);

        let messages: Vec<types::MessageWithId> = bob
            .owner
            .view(bob.contract.id(), "view_thread")
            .args_json(serde_json::json!({
                "sender": "chat.alice.test.near",
            }))
            .await
            .unwrap()
            .json()
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages.first().unwrap().message.content, "Hello, Bob!");
    }

    async fn setup_messenger_contract(
        account_name: &str,
        worker: &workspaces::Worker<workspaces::network::Sandbox>,
    ) -> MessengerInstance {
        // This is needed because of a quirk of how `cargo-near` works. It doesn't handle
        // cargo workspaces properly yet.
        tokio::fs::create_dir_all("../target/near/near_messenger")
            .await
            .unwrap();
        let contract_bytes = utils::cargo::build_contract("../contract").await.unwrap();

        let (_, sk) = worker.dev_generate().await;
        let account = worker
            .create_tla(account_name.parse().unwrap(), sk)
            .await
            .unwrap()
            .into_result()
            .unwrap();
        let messenger_account = account
            .create_subaccount("chat")
            .initial_balance(50_000_000_000_000_000_000_000_000)
            .transact()
            .await
            .unwrap()
            .into_result()
            .unwrap();
        let contract = messenger_account
            .deploy(&contract_bytes)
            .await
            .unwrap()
            .into_result()
            .unwrap();
        account
            .call(contract.id(), "new")
            .transact()
            .await
            .unwrap()
            .into_result()
            .unwrap();

        MessengerInstance {
            contract,
            owner: account,
        }
    }

    struct MessengerInstance {
        pub contract: workspaces::Contract,
        pub owner: workspaces::Account,
    }
}
