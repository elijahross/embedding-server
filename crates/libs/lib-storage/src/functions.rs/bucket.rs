use crate::error::{Error, Result};
use aws_sdk_s3::types::{BucketLocationConstraint, CreateBucketConfiguration};
use aws_sdk_s3::Client;

pub async fn create_s3_bucket(client: &Client, bucket: &str) -> Result<()> {
    let region = client
        .config()
        .region()
        .ok_or_else(|| Error::ProcessFail("Missing region".to_string()))?
        .as_ref();

    let config = if region != "us-east-1" {
        Some(
            CreateBucketConfiguration::builder()
                .location_constraint(BucketLocationConstraint::from(region))
                .build(),
        )
    } else {
        None
    };

    let mut request = client.create_bucket().bucket(bucket);
    if let Some(cfg) = config {
        request = request.create_bucket_configuration(cfg);
    }

    request.send().await.map_err(|err| {
        let full = format!("{:?}", err); // full debug print of the error
        Error::ProcessFail(format!("Failed to create bucket: {full}"))
    })?;

    Ok(())
}

pub async fn get_total_bucket_size(client: &Client, bucket: &str) -> Result<u64> {
    let mut total_size: u64 = 0;
    let mut continuation_token: Option<String> = None;

    loop {
        let resp = client
            .list_objects_v2()
            .bucket(bucket)
            .set_continuation_token(continuation_token.clone())
            .send()
            .await
            .map_err(|err| Error::ProcessFail(format!("{err}")))?;

        let data = resp.clone().contents.unwrap_or([].into());
        total_size += data
            .iter()
            .map(|obj| obj.size().unwrap() as u64)
            .sum::<u64>();

        if let Some(token) = resp.next_continuation_token() {
            continuation_token = Some(token.to_string());
        } else {
            break;
        }
    }

    Ok(total_size)
}

// region: Unit Test
#[cfg(test)]
mod test {
    use super::*;
    use crate::create_aws_client;

    #[tokio::test]
    async fn test_get_total_bucket_size() {
        let client = create_aws_client().await;
        let bucket_name = "your-bucket-name";

        match get_total_bucket_size(&client, "uploaded-files").await {
            Ok(size) => println!("Total size of bucket {}: {} bytes", bucket_name, size),
            Err(err) => eprintln!("Error: {}", err),
        }
    }

    #[tokio::test]
    async fn test_create_s3_bucket() -> Result<()> {
        let client = create_aws_client().await;
        let bucket_name = "test-bucket-1234567890"; // Replace with a unique bucket name

        match create_s3_bucket(&client, bucket_name).await {
            Ok(_) => println!("Bucket {} created successfully", bucket_name),
            Err(err) => eprintln!("Error: {}", err),
        }

        Ok(())
    }
}
// endregion: Unit Test