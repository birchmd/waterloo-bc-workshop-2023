use crate::{
    config::Config,
    types::{ManagerMessage, ManagerMessageKind, ReceiptHandlerMessage, ShutdownSignal},
};
use near_jsonrpc_client::{
    errors::{JsonRpcError, JsonRpcServerError},
    methods, JsonRpcClient,
};
use near_messenger::events::Event;
use near_primitives::{
    hash::CryptoHash,
    types::AccountId,
    views::{ExecutionOutcomeWithIdView, ReceiptEnumView},
};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

/// An "actor" which represents a background task to poll the Near RPC
/// at regular intervals for new blocks.
pub struct ReceiptHandler {
    id: String,
    client: JsonRpcClient,
    retry_frequency: Duration,
    manager_channel: Sender<ManagerMessage>,
    incoming_channel: Receiver<ReceiptHandlerMessage>,
    target_account: AccountId,
    max_retry_count: usize,
    events_output_path: PathBuf,
}

impl ReceiptHandler {
    pub async fn new(
        config: &Config,
        manager_channel: Sender<ManagerMessage>,
        id_no: usize,
    ) -> anyhow::Result<(Self, Sender<ReceiptHandlerMessage>)> {
        let id = format!("ReceiptHandler_{id_no}");
        let target_account = config.target_account.parse()?;
        let max_retry_count = config.max_download_retry.into();
        let retry_frequency = Duration::from_millis(config.polling_frequency_ms);
        let client = JsonRpcClient::new_client().connect(&config.near_rpc_url);
        let events_output_path = Path::new(&config.events_output_path).into();
        tokio::fs::OpenOptions::default()
            .create(true)
            .append(true)
            .open(&events_output_path)
            .await?;

        let (sender, incoming_channel) = mpsc::channel(100);

        let this = Self {
            id,
            client,
            retry_frequency,
            manager_channel,
            incoming_channel,
            target_account,
            max_retry_count,
            events_output_path,
        };

        Ok((this, sender))
    }

    pub fn start(mut self) -> JoinHandle<anyhow::Result<()>> {
        tokio::task::spawn(async move {
            while let Some(message) = self.incoming_channel.recv().await {
                match message {
                    ReceiptHandlerMessage::Handle {
                        receipt,
                        next_block_hash,
                    } => {
                        if receipt.receiver_id != self.target_account {
                            continue;
                        }
                        // Nothing to do with Data-type receipts.
                        // We only care about Action-type receipts.
                        if let ReceiptEnumView::Data { .. } = receipt.receipt {
                            continue;
                        };
                        tracing::info!("Downloading outcome for receipt {:?} included in the parent of block {:?}", receipt.receipt_id, next_block_hash);
                        match download_outcome_with_retry(
                            &self.client,
                            receipt.receipt_id,
                            &receipt.receiver_id,
                            next_block_hash,
                            self.retry_frequency,
                            self.max_retry_count,
                        )
                        .await
                        {
                            Ok(outcome) => {
                                let events =
                                    outcome.outcome.logs.into_iter().filter_map(parse_event);
                                for event in events {
                                    // Events from the chat contract are handled here
                                    // EXERCISE: can you make the contents of a received message appear in the output as well?
                                    if let Err(e) = self.handle_event(event).await {
                                        tracing::warn!("Error while handling event: {:?}", e);
                                        self.send_manager_message(ManagerMessageKind::Shutdown(
                                            ShutdownSignal,
                                        ))
                                        .await
                                        .ok();
                                        return Err(anyhow::anyhow!("Failed to handle events"));
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to download outcome: {:?}", e);
                                self.send_manager_message(ManagerMessageKind::Shutdown(
                                    ShutdownSignal,
                                ))
                                .await
                                .ok();
                                return Err(anyhow::anyhow!("Failed to download outcome"));
                            }
                        };
                    }
                    ReceiptHandlerMessage::Shutdown(ShutdownSignal) => {
                        tracing::info!("ReceiptHandler received ShutdownSignal");
                        break;
                    }
                }
            }
            Ok(())
        })
    }

    async fn send_manager_message(&self, kind: ManagerMessageKind) -> anyhow::Result<()> {
        let message = ManagerMessage {
            worker_id: self.id.clone(),
            kind,
        };
        self.manager_channel.send(message).await?;
        Ok(())
    }

    async fn handle_event(&self, event: Event<'static>) -> anyhow::Result<()> {
        // Events from the messenger contract are handled here.
        // EXERCISE: can you make the contents of a received message appear in the output as well?
        tracing::debug!("Event: {:?}", event);
        self.write_event(&event).await?;
        Ok(())
    }

    async fn write_event(&self, event: &Event<'static>) -> anyhow::Result<()> {
        let mut file = tokio::fs::OpenOptions::default()
            .append(true)
            .open(&self.events_output_path)
            .await?;
        let content = serde_json::to_string_pretty(event)?;
        file.write_all(content.as_bytes()).await?;
        file.write_all(b"\n").await?;
        Ok(())
    }
}

async fn download_outcome_with_retry(
    client: &JsonRpcClient,
    receipt_id: CryptoHash,
    receiver_id: &AccountId,
    block_hash: CryptoHash,
    retry_frequency: Duration,
    max_retries: usize,
) -> anyhow::Result<ExecutionOutcomeWithIdView> {
    for _ in 0..max_retries {
        match download_outcome(client, receipt_id, receiver_id, block_hash).await {
            Ok(outcome) => return Ok(outcome),
            Err(e) => {
                tracing::warn!("Failed to download outcome: {:?}", e);
                tokio::time::sleep(retry_frequency).await;
            }
        }
    }
    Err(anyhow::anyhow!("Failed to download outcome"))
}

async fn download_outcome(
    client: &JsonRpcClient,
    receipt_id: CryptoHash,
    receiver_id: &AccountId,
    mut block_hash: CryptoHash,
) -> anyhow::Result<ExecutionOutcomeWithIdView> {
    loop {
        let request = methods::light_client_proof::RpcLightClientExecutionProofRequest {
            id: near_primitives::types::TransactionOrReceiptId::Receipt {
                receipt_id,
                receiver_id: receiver_id.clone(),
            },
            light_client_head: block_hash,
        };
        let maybe_response = client.call(request).await;
        match maybe_response {
            Ok(response) => {
                return Ok(response.outcome_proof);
            }
            Err(JsonRpcError::ServerError(JsonRpcServerError::InternalError { info }))
                if info.is_some() =>
            {
                // There is a special error where the RPC will not tell us the outcome because we
                // have not given a recent enough hash with our query. We don't care; we just
                // want the outcome. So let's hack it and parse the block hash it wants from
                // the error message and try again.
                let err_message = info.unwrap();
                if err_message.contains("is ahead of head block") {
                    if let Some(hash) = try_parse_block_hash_from_err_message(&err_message) {
                        block_hash = hash;
                        continue;
                    };
                }
                return Err(anyhow::anyhow!("internal jsonrpc error: {:?}", err_message));
            }
            Err(other) => {
                return Err(other.into());
            }
        }
    }
}

fn try_parse_block_hash_from_err_message(msg: &str) -> Option<CryptoHash> {
    let msg = msg.strip_prefix("block ")?;
    let hash_b58 = msg.split(' ').next()?;
    CryptoHash::from_str(hash_b58).ok()
}

fn parse_event(log: String) -> Option<Event<'static>> {
    let json_str = log.strip_prefix("EVENT_JSON:")?;
    let event = serde_json::from_str(json_str).ok()?;
    Some(event)
}
