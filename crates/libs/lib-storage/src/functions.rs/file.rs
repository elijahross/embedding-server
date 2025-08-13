use crate::error::{Error, Result};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use std::time::Duration;

pub async fn upload_file(
    client: &Client,
    bucket: &str,
    key: &str,
    data: Vec<u8>,
) -> Result<String> {
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from(data))
        .send()
        .await
        .map_err(|_| Error::ProcessFail("Failed to upload file".into()))?;

    Ok(key.to_string())
}

pub async fn list_files_in_bucket(
    client: &Client,
    bucket: &str,
    prefix: Option<&str>,
) -> Result<Vec<String>> {
    let mut keys = Vec::new();

    let resp = client
        .list_objects_v2()
        .bucket(bucket)
        .set_prefix(prefix.map(String::from))
        .send()
        .await
        .map_err(|_| Error::ProcessFail("Failed to list files".into()))?;

    let data = resp.clone().contents.unwrap_or([].into());
    for obj in data {
        if let Some(key) = obj.key() {
            keys.push(key.to_string());
        }
    }

    Ok(keys)
}

pub async fn delete_file(client: &Client, bucket: &str, key: &str) -> Result<()> {
    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|_| Error::ProcessFail("Failed to delete file".into()))?;

    Ok(())
}

pub async fn generate_presigned_url(
    client: &Client,
    bucket: &str,
    key: &str,
    expiration_in_seconds: i64,
) -> Result<String> {
    let presigning_config =
        PresigningConfig::expires_in(Duration::from_secs(expiration_in_seconds as u64))
            .map_err(|_| Error::ErrorSigningUrl)?;

    let presigned_url = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(presigning_config)
        .await
        .map_err(|_| Error::ProcessFail("Failed to generate presigned URL".into()))?;

    Ok(presigned_url.uri().to_string())
}

pub async fn rename_file(
    client: &Client,
    bucket: &str,
    old_key: &str,
    new_key: &str,
) -> Result<()> {
    // Copy the object to the new key
    client
        .copy_object()
        .copy_source(format!("{}/{}", bucket, old_key))
        .bucket(bucket)
        .key(new_key)
        .send()
        .await
        .map_err(|_| Error::ProcessFail("Failed to copy file".into()))?;

    // Delete the old object
    delete_file(client, bucket, old_key).await?;

    Ok(())
}