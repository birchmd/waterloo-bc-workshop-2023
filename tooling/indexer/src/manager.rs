use crate::types::{
    ChunkDownloaderMessage, ManagerMessage, ManagerMessageKind, ReceiptHandlerMessage,
    ShutdownSignal,
};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    task::JoinHandle,
};

pub struct Manager {
    incoming_channel: Receiver<ManagerMessage>,
    block_downloader_channel: Sender<ShutdownSignal>,
    chunk_downloader_channels: Vec<Sender<ChunkDownloaderMessage>>,
    receipt_handler_channel: Sender<ReceiptHandlerMessage>,
}

impl Manager {
    pub fn new(
        incoming_channel: Receiver<ManagerMessage>,
        block_downloader_channel: Sender<ShutdownSignal>,
        chunk_downloader_channels: Vec<Sender<ChunkDownloaderMessage>>,
        receipt_handler_channel: Sender<ReceiptHandlerMessage>,
    ) -> Self {
        Self {
            incoming_channel,
            block_downloader_channel,
            chunk_downloader_channels,
            receipt_handler_channel,
        }
    }

    pub fn start(mut self) -> JoinHandle<anyhow::Result<()>> {
        tokio::task::spawn(async move {
            let mut chunk_downloaders = self.chunk_downloader_channels.iter().cycle();
            while let Some(message) = self.incoming_channel.recv().await {
                tracing::debug!("Manager received a message from {}", message.worker_id);
                match message.kind {
                    ManagerMessageKind::NewBlock {
                        block,
                        next_block_hash,
                    } => {
                        let block_hash = block.header.hash;
                        tracing::debug!("Received block {:?}", block_hash);
                        for (chunk, included) in
                            block.chunks.iter().zip(block.header.chunk_mask.iter())
                        {
                            if !included {
                                continue;
                            }
                            // Unwrap is safe because we cycle the iterator above
                            let chunk_downloader = chunk_downloaders.next().unwrap();
                            chunk_downloader
                                .send(ChunkDownloaderMessage::Download {
                                    chunk_hash: chunk.chunk_hash,
                                    next_block_hash,
                                })
                                .await
                                .ok();
                        }
                    }
                    ManagerMessageKind::NewChunk {
                        chunk,
                        next_block_hash,
                    } => {
                        tracing::debug!("Received chunk {:?}", chunk.header.chunk_hash);
                        for receipt in chunk.receipts {
                            self.receipt_handler_channel
                                .send(ReceiptHandlerMessage::Handle {
                                    receipt: Box::new(receipt),
                                    next_block_hash,
                                })
                                .await
                                .ok();
                        }
                    }
                    ManagerMessageKind::Shutdown(ShutdownSignal) => {
                        tracing::info!("Manager: ShutdownSignal received");
                        self.block_downloader_channel
                            .send(ShutdownSignal)
                            .await
                            .ok();
                        for channel in self.chunk_downloader_channels.iter() {
                            channel
                                .send(ChunkDownloaderMessage::Shutdown(ShutdownSignal))
                                .await
                                .ok();
                        }
                        self.receipt_handler_channel
                            .send(ReceiptHandlerMessage::Shutdown(ShutdownSignal))
                            .await
                            .ok();
                        break;
                    }
                }
            }
            Ok(())
        })
    }
}
