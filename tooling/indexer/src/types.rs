use near_primitives::{
    hash::CryptoHash,
    views::{BlockView, ChunkView, ReceiptView},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownSignal;

#[derive(Debug)]
pub struct ManagerMessage {
    pub worker_id: String,
    pub kind: ManagerMessageKind,
}

#[derive(Debug)]
pub enum ManagerMessageKind {
    NewBlock {
        block: Box<BlockView>,
        // `block` is the parent of the block with hash `next_block_hash`.
        // `next_block_hash` needs to be known because we can only query outcome
        // of a receipt from the perspective of the block after it was included.
        next_block_hash: CryptoHash,
    },
    NewChunk {
        chunk: Box<ChunkView>,
        // `chunk_hash` was included in `block(next_block_hash).parent`
        // `next_block_hash` needs to be known because we can only query outcome
        // of a receipt from the perspective of the block after it was included.
        next_block_hash: CryptoHash,
    },
    Shutdown(ShutdownSignal),
}

#[derive(Debug)]
pub enum ChunkDownloaderMessage {
    Shutdown(ShutdownSignal),
    Download {
        chunk_hash: CryptoHash,
        // `chunk_hash` was included in `block(next_block_hash).parent`
        // `next_block_hash` needs to be known because we can only query outcome
        // of a receipt from the perspective of the block after it was included.
        next_block_hash: CryptoHash,
    },
}

#[derive(Debug)]
pub enum ReceiptHandlerMessage {
    Shutdown(ShutdownSignal),
    Handle {
        receipt: Box<ReceiptView>,
        // The receipt was included in the parent of the block with hash `next_block_hash`.
        // `next_block_hash` needs to be known because we can only query outcome
        // of a receipt from the perspective of the block after it was included.
        next_block_hash: CryptoHash,
    },
}
