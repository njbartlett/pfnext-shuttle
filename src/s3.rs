//! Object storage in a DigitalOcean Spaces bucket, through its S3-compatible
//! API. Public objects are served from the bucket's CDN.
//!
//! Configured from the environment: S3_BUCKET names the bucket, AWS_REGION is
//! the Spaces region (e.g. lon1), and AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY
//! are the Spaces access key, read by the AWS SDK itself.
use std::env;

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::ObjectCannedAcl;
use rand::distributions::Alphanumeric;
use rand::Rng;

/// Length of the random names given to uploaded objects
const RANDOM_NAME_LEN: usize = 12;

pub struct S3Store {
    client: aws_sdk_s3::Client,
    bucket: String,
    region: String
}

impl S3Store {
    pub async fn new_from_env() -> Result<Self, String> {
        let bucket = env::var("S3_BUCKET").map_err(|e| format!("{e}: S3_BUCKET"))?;
        let region = env::var("AWS_REGION").map_err(|e| format!("{e}: AWS_REGION"))?;
        let s3_config = aws_config::from_env()
            .endpoint_url(format!("https://{region}.digitaloceanspaces.com"))
            .load()
            .await;
        let client = aws_sdk_s3::Client::new(&s3_config);
        Ok(Self { client, bucket, region })
    }

    pub async fn upload(
        &self,
        bytes: ByteStream,
        content_type: &str,
        key: &str,
        public: bool
    ) -> Result<(), String> {
        let acl = if public {
            ObjectCannedAcl::PublicRead
        } else {
            ObjectCannedAcl::Private
        };

        self.client.put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .acl(acl)
            .body(bytes)
            .send()
            .await
            .map_err(|e| format!("Failed to upload {key} to S3: {e}"))?;
        Ok(())
    }

    /// The CDN URL of a public object
    pub fn public_url(&self, key: &str) -> String {
        public_url(&self.bucket, &self.region, key)
    }
}

fn public_url(bucket: &str, region: &str, key: &str) -> String {
    let encoded_key: Vec<String> = key.split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect();
    format!("https://{bucket}.{region}.cdn.digitaloceanspaces.com/{}", encoded_key.join("/"))
}

/// A random alphanumeric name for a new object
pub fn random_name() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(RANDOM_NAME_LEN)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_url() {
        assert_eq!("https://photos.lon1.cdn.digitaloceanspaces.com/pfnext/blog/images/abc",
            public_url("photos", "lon1", "pfnext/blog/images/abc"));
        assert_eq!("https://b.lon1.cdn.digitaloceanspaces.com/a%20b/c%26d", public_url("b", "lon1", "a b/c&d"));
    }

    #[test]
    fn test_random_name() {
        let name = random_name();
        assert_eq!(RANDOM_NAME_LEN, name.len());
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(name, random_name());
    }
}
