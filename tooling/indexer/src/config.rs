pub struct Config {
    pub events_output_path: String,
    pub log_level: String,
    pub max_download_retry: u8,
    pub near_rpc_url: String,
    pub num_chunk_downloaders: u8,
    pub polling_frequency_ms: u64,
    pub target_account: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            events_output_path: "events.log".into(),
            log_level: "debug".into(),
            max_download_retry: 20,
            near_rpc_url: "https://rpc.testnet.near.org".into(),
            num_chunk_downloaders: 4,
            polling_frequency_ms: 1_200,
            target_account: "chat.waterloo_bc_demo_2023.testnet".into(),
        }
    }
}
