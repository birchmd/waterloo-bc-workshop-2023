# Indexer for `near-messenger`

The purpose of an indexer is to get relevant data from the blockchain for a specific app.
For example, an indexer could be used to provide a stream of events from a smart contract to the web or mobile frontend of an app.

Indexers typically use a local running node (one that is connected to the blockchain's peer-to-peer network), or a data service provider like [Near Data Lake](https://docs.near.org/concepts/advanced/near-lake-framework). But Near nodes require a large amount of disk space, and Near Data Lake uses AWS, which costs money. So for the sake of this small example I decided to slightly abuse the public Near RPC to get the data we need to build an indexer. Keep in mind that the concepts here apply regardless of where the data comes from, so it would be relatively easy to adapt this to work with a local Near node or Near Data Lake connection.

## Architecture

There are four main kinds of processes in the system:

1. manager,
2. block downloader,
3. chunk downloader (there are multiple instances of this kind),
4. receipt handler.

Each one runs in parallel with the others.
This is simply a performance improvement; the process could run sequentially (first download a new blocks, then all its chunks, then process any relevant receipts therein), but would be much slower.

The purpose of each process is described in the sections below.

### Manager

This process supervises the whole indexer.
It is responsible for delegating work to the other processes, and telling them to shutdown when the program is being closed (e.g. in the case of an error being encountered).

### Block Downloader

As the name implies, the purpose of this process is to download blocks.
It periodically polls the Near RPC to check if there is a new block and if there is then downloads it and sends it to the manager.
The manager will then assign the various chunk downloaders to download the chunks from this block.
If we were not using the RPC as our data source then this process would be replaced with a connection to a Near node or Data Lake instead.

### Chunk Downloader

On Near blocks do not contain the data about transactions; chunks do.
Blocks only give information about what new chunks are available.
The reason for this is because of Near's sharding (you can read more about that [here](https://near.org/papers/nightshade/)).
Therefore, we need separate processes to download the chunk data for each block.
The chunk downloaders fulfil this role.
If we were not using the RPC as our data source then these processes may not need to exist, depending on how the data is factored in the data source we were using (for example [near-indexer-framework](https://docs.near.org/concepts/advanced/near-indexer-framework) includes all block and chunk data into a single message).

### Receipt Handler

This process extracts relevant data from all the transaction receipts.
It may need to do more or less work depending on the data source.
For example, since we are using the RPC we need to make additional calls to get the outcome of the receipts, whereas other data sources may include the outcomes in the initial message.
Regardless, this is where the "business logic" of our indexer lives.
