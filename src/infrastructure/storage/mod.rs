//! S3 client (SeaweedFS lokal / R2) — path-style, kredensial statis dari env.
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::Client;

pub struct Storage {
    pub client: Client,
    /// Client utk presign URL — host PUBLIK (diakses device/browser).
    pub client_public: Client,
    pub bucket: String,
}

pub async fn build(s3: &crate::config::S3Config) -> Storage {
    let mk = |ep: &str| {
        aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .endpoint_url(ep)
            .credentials_provider(Credentials::new(
                &s3.access_key, &s3.secret_key, None, None, "mq-static",
            ))
            .force_path_style(true) // SeaweedFS/MinIO style
            .build()
    };
    // runtime ops (HEAD/GET range) = endpoint internal (menghindari Cloudflare BIC
    // yang menolak UA aws-sdk-rust); presign = host publik utk device/browser.
    let internal_ep = s3.endpoint_internal.as_deref().unwrap_or(&s3.endpoint);
    Storage {
        client: Client::from_conf(mk(internal_ep)),
        client_public: Client::from_conf(mk(&s3.endpoint)),
        bucket: s3.bucket.clone(),
    }
}

impl Storage {
    /// Presigned PUT (upload klien langsung) — short-lived (≤15 menit, keputusan #3 Bagian III).
    pub async fn presign_put(&self, key: &str, mime: &str, secs: u64) -> Result<String, String> {
        use aws_sdk_s3::presigning::PresigningConfig;
        let cfg = PresigningConfig::expires_in(std::time::Duration::from_secs(secs))
            .map_err(|e| e.to_string())?;
        self.client_public
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(mime)
            .presigned(cfg)
            .await
            .map(|p| p.uri().to_string())
            .map_err(|e| e.to_string())
    }

    /// Presigned GET (regenerate on demand) — storage_key tidak pernah keluar.
    pub async fn presign_get(&self, key: &str, secs: u64) -> Result<String, String> {
        use aws_sdk_s3::presigning::PresigningConfig;
        let cfg = PresigningConfig::expires_in(std::time::Duration::from_secs(secs))
            .map_err(|e| e.to_string())?;
        self.client_public
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(cfg)
            .await
            .map(|p| p.uri().to_string())
            .map_err(|e| e.to_string())
    }

    pub async fn head_size(&self, key: &str) -> Result<Option<i64>, String> {
        match self.client.head_object().bucket(&self.bucket).key(key).send().await {
            Ok(h) => Ok(h.content_length),
            Err(e) => Err(format!("head: {e}")),
        }
    }

    /// 64 byte pertama (magic bytes — validasi server-side, keputusan #17).
    pub async fn first_bytes(&self, key: &str) -> Result<Vec<u8>, String> {
        let obj = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .range("bytes=0-63")
            .send()
            .await
            .map_err(|e| format!("get range: {e}"))?;
        let data = obj.body.collect().await.map_err(|e| e.to_string())?;
        Ok(data.into_bytes().to_vec())
    }
}
