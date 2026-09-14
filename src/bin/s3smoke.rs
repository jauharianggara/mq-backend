//! Debug SDK S3 → SeaweedFS: head + get-range, error chain lengkap. `cargo run --bin s3smoke`
#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let cfg = mq_backend_lib::config_for_smoke();
    let s3 = mq_backend_lib::storage_for_smoke(&cfg).await;
    // ambil satu media READY/UPLOADING dari DB utk key nyata
    let pool = mq_backend_lib::pool_for_smoke(&cfg).await;
    let key: Option<(String,)> = sqlx::query_as(
        "SELECT storage_key FROM media ORDER BY id DESC LIMIT 1")
        .fetch_one(&pool).await.ok();
    let key = key.map(|k| k.0).unwrap_or_else(|| "audio/2026/09/tidak-ada.m4a".into());
    println!("HEAD key: {key}");
    match s3.client.head_object().bucket(&s3.bucket).key(&key).send().await {
        Ok(h) => println!("HEAD OK content_length={:?}", h.content_length),
        Err(e) => {
            println!("HEAD ERR display: {e}");
            println!("HEAD ERR debug: {e:?}");
        }
    }
    match s3.client.get_object().bucket(&s3.bucket).key(&key).range("bytes=0-15").send().await {
        Ok(g) => {
            let b = g.body.collect().await.unwrap().into_bytes();
            println!("GET RANGE OK: {:02X?}", &b[..b.len().min(8)]);
        }
        Err(e) => println!("GET ERR: {e}"),
    }
}
