use crate::{
    config::Config,
    types::{ChunkDownloaderMessage, ManagerMessage, ManagerMessageKind, ShutdownSignal},
};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_jsonrpc_primitives::types::chunks::ChunkReference;
use near_primitives::{hash::CryptoHash, views::ChunkView};
use std::time::Duration;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

/// An "actor" which represents a background task to poll the Near RPC
/// at regular intervals for new blocks.
pub struct ChunkDownloader {
    id: String,
    client: JsonRpcClient,
    retry_frequency: Duration,
    manager_channel: Sender<ManagerMessage>,
    incoming_channel: Receiver<ChunkDownloaderMessage>,
    max_retry_count: usize,
}

impl ChunkDownloader {
    pub fn new(
        config: &Config,
        manager_channel: Sender<ManagerMessage>,
        id_no: usize,
    ) -> (Self, Sender<ChunkDownloaderMessage>) {
        let id = format!("ChunkDownloader_{id_no}");
        let max_retry_count = config.max_download_retry.into();
        let retry_frequency = Duration::from_millis(config.polling_frequency_ms);
        let client = JsonRpcClient::new_client().connect(&config.near_rpc_url);

        let (sender, incoming_channel) = mpsc::channel(100);

        let this = Self {
            id,
            client,
            retry_frequency,
            manager_channel,
            incoming_channel,
            max_retry_count,
        };

        (this, sender)
    }

    pub fn start(mut self) -> JoinHandle<anyhow::Result<()>> {
        tokio::task::spawn(async move {
            while let Some(message) = self.incoming_channel.recv().await {
                tracing::debug!("{} received a message from the Manager", self.id);
                match message {
                    ChunkDownloaderMessage::Download {
                        chunk_hash,
                        next_block_hash: block_hash,
                    } => {
                        match download_chunk_with_retry(
                            &self.client,
                            chunk_hash,
                            self.retry_frequency,
                            self.max_retry_count,
                        )
                        .await
                        {
                            Ok(chunk) => {
                                if let Err(e) = self
                                    .send_manager_message(ManagerMessageKind::NewChunk {
                                        chunk: Box::new(chunk),
                                        next_block_hash: block_hash,
                                    })
                                    .await
                                {
                                    tracing::error!(
                                        "ChunkDownloader failed to communicate with Manager."
                                    );
                                    return Err(e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to download chunk: {:?}", e);
                                self.send_manager_message(ManagerMessageKind::Shutdown(
                                    ShutdownSignal,
                                ))
                                .await
                                .ok();
                                return Err(anyhow::anyhow!("Failed to download chunks"));
                            }
                        }
                    }
                    ChunkDownloaderMessage::Shutdown(ShutdownSignal) => {
                        tracing::info!("ChunkDownloader received ShutdownSignal");
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
}

async fn download_chunk_with_retry(
    client: &JsonRpcClient,
    chunk_hash: CryptoHash,
    retry_frequency: Duration,
    max_retries: usize,
) -> anyhow::Result<ChunkView> {
    for _ in 0..max_retries {
        match download_chunk(client, chunk_hash).await {
            Ok(chunk) => return Ok(chunk),
            Err(e) => {
                tracing::warn!("Failed to download chunk: {:?}", e);
                tokio::time::sleep(retry_frequency).await;
            }
        }
    }
    Err(anyhow::anyhow!("Failed to download chunk"))
}

async fn download_chunk(client: &JsonRpcClient, chunk_id: CryptoHash) -> anyhow::Result<ChunkView> {
    let request = methods::chunk::RpcChunkRequest {
        chunk_reference: ChunkReference::ChunkHash { chunk_id },
    };
    let chunk = client.call(request).await?;
    Ok(chunk)
}
