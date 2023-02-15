use crate::{
    config::Config,
    types::{ManagerMessage, ManagerMessageKind, ShutdownSignal},
};
use near_jsonrpc_client::{methods, JsonRpcClient};
use near_primitives::{
    hash::CryptoHash,
    types::{BlockId, BlockReference, Finality},
    views::BlockView,
};
use std::time::Duration;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

/// An "actor" which represents a background task to poll the Near RPC
/// at regular intervals for new blocks.
pub struct BlockDownloader {
    id: String,
    client: JsonRpcClient,
    polling_frequency: Duration,
    manager_channel: Sender<ManagerMessage>,
    shutdown_channel: Receiver<ShutdownSignal>,
    last_seen_block: BlockView,
    retry_count: usize,
    max_retry_count: usize,
}

impl BlockDownloader {
    pub async fn new(
        config: &Config,
        manager_channel: Sender<ManagerMessage>,
        id_no: usize,
    ) -> anyhow::Result<(Self, Sender<ShutdownSignal>)> {
        let id = format!("BlockDownloader_{id_no}");
        let max_retry_count = config.max_download_retry.into();
        let polling_frequency = Duration::from_millis(config.polling_frequency_ms);
        let client = JsonRpcClient::new_client().connect(&config.near_rpc_url);
        let last_seen_block = get_latest_block(&client).await?;

        let (shutdown_sender, shutdown_channel) = mpsc::channel(5);

        let this = Self {
            id,
            client,
            last_seen_block,
            manager_channel,
            shutdown_channel,
            polling_frequency,
            retry_count: 0,
            max_retry_count,
        };

        Ok((this, shutdown_sender))
    }

    pub fn start(mut self) -> JoinHandle<anyhow::Result<()>> {
        tokio::task::spawn(async move {
            loop {
                let maybe_shutdown =
                    tokio::time::timeout(self.polling_frequency, self.shutdown_channel.recv())
                        .await;
                match maybe_shutdown {
                    Ok(Some(ShutdownSignal)) => {
                        tracing::info!("BlockDownloader received ShutdownSignal");
                        break;
                    }
                    Ok(None) => {
                        tracing::warn!("BlockDownloader shutdown channel closed.");
                        break;
                    }
                    Err(_) => {
                        // Err(_) means we hit the polling frequency before receiving a shutdown message.
                        // So let's see if there is a new block to download.
                        tracing::debug!("BlockDownloader beginning polling cycle");
                        let maybe_latest_block = get_latest_block(&self.client).await;
                        let maybe_blocks = match maybe_latest_block {
                            Ok(block) => {
                                // If the block has not updated then we wait for
                                // the next polling cycle.
                                if block.header.hash == self.last_seen_block.header.hash {
                                    continue;
                                }
                                download_block_chain(
                                    &self.client,
                                    block,
                                    self.last_seen_block.header.hash,
                                )
                                .await
                            }
                            Err(e) => Err(e),
                        };
                        match maybe_blocks {
                            Ok(blocks) => {
                                self.retry_count = 0;

                                for block in blocks {
                                    let message = ManagerMessageKind::NewBlock {
                                        block: Box::new(self.last_seen_block),
                                        next_block_hash: block.header.hash,
                                    };
                                    self.last_seen_block = block;
                                    if let Err(e) = self.send_manager_message(message).await {
                                        tracing::error!(
                                            "BlockDownloader failed to communicate with Manager."
                                        );
                                        return Err(e);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("BlockDownloader failed to fetch blocks: {:?}", e);
                                self.retry_count += 1;
                                if self.retry_count >= self.max_retry_count {
                                    self.send_manager_message(ManagerMessageKind::Shutdown(
                                        ShutdownSignal,
                                    ))
                                    .await
                                    .ok();
                                    return Err(anyhow::anyhow!("Failed to fetch blocks"));
                                }
                            }
                        }
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

/// Downloads blocks, following parent hashes until the `target_parent` is reached.
async fn download_block_chain(
    client: &JsonRpcClient,
    current_block: BlockView,
    target_parent: CryptoHash,
) -> anyhow::Result<Vec<BlockView>> {
    let mut blocks = vec![current_block];

    while blocks.last().unwrap().header.prev_hash != target_parent {
        let hash = blocks.last().unwrap().header.prev_hash;
        let block_request = methods::block::RpcBlockRequest {
            block_reference: BlockReference::BlockId(BlockId::Hash(hash)),
        };
        tracing::debug!("JsonRpcClient call to download block {:?}", hash);
        let block = client.call(block_request).await?;
        blocks.push(block);
    }
    // Reverse the order of blocks so they are ordered oldest to newest
    // instead of the other way around.
    blocks.reverse();

    Ok(blocks)
}

async fn get_latest_block(client: &JsonRpcClient) -> anyhow::Result<BlockView> {
    let block_request = methods::block::RpcBlockRequest {
        block_reference: BlockReference::Finality(Finality::DoomSlug),
    };
    tracing::debug!("JsonRpcClient call to download latest block");
    let block = client.call(block_request).await?;
    Ok(block)
}
