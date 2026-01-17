use aws_config::BehaviorVersion;
use aws_sdk_s3::{config::Credentials, config::Region, presigning::PresigningConfig, Client};
use std::time::Duration;

#[derive(Clone)]
pub struct StorageClient {
    client: Client,
    bucket: String,
}

impl StorageClient {
    pub async fn new(config: StorageConfig) -> Result<Self, String> {
        let credentials = Credentials::new(
            config.access_key,
            config.secret_key,
            None,
            None,
            "newral",
        );
        let region = Region::new(config.region);
        let shared = aws_config::defaults(BehaviorVersion::latest())
            .region(region)
            .credentials_provider(credentials)
            .endpoint_url(config.endpoint)
            .load()
            .await;
        let s3_config = aws_sdk_s3::config::Builder::from(&shared)
            .force_path_style(config.force_path_style)
            .build();
        Ok(Self {
            client: Client::from_conf(s3_config),
            bucket: config.bucket,
        })
    }

    pub async fn ensure_project_prefix(&self, prefix: &str) -> Result<(), String> {
        self.ensure_bucket().await?;
        let key = format!("{}/.init", prefix.trim_end_matches('/'));
        self.client
            .put_object()
            .bucket(self.bucket.as_str())
            .key(key)
            .body(aws_sdk_s3::primitives::ByteStream::from(vec![]))
            .send()
            .await
            .map_err(|err| format!("put object failed: {err}"))?;
        Ok(())
    }

    pub async fn presign_get(&self, key: &str, ttl_secs: u64) -> Result<String, String> {
        let presigned = self
            .client
            .get_object()
            .bucket(self.bucket.as_str())
            .key(key)
            .presigned(PresigningConfig::expires_in(Duration::from_secs(ttl_secs)).map_err(
                |err| format!("presign config failed: {err}"),
            )?)
            .await
            .map_err(|err| format!("presign failed: {err}"))?;
        Ok(presigned.uri().to_string())
    }

    pub async fn put_object(&self, key: &str, body: Vec<u8>) -> Result<(), String> {
        self.ensure_bucket().await?;
        self.client
            .put_object()
            .bucket(self.bucket.as_str())
            .key(key)
            .body(aws_sdk_s3::primitives::ByteStream::from(body))
            .send()
            .await
            .map_err(|err| format!("put object failed: {err}"))?;
        Ok(())
    }

    async fn ensure_bucket(&self) -> Result<(), String> {
        let exists = self
            .client
            .head_bucket()
            .bucket(self.bucket.as_str())
            .send()
            .await
            .is_ok();
        if !exists {
            self.client
                .create_bucket()
                .bucket(self.bucket.as_str())
                .send()
                .await
                .map_err(|err| format!("create bucket failed: {err}"))?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct StorageConfig {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub force_path_style: bool,
}
