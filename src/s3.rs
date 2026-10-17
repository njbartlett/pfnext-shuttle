//! Object storage in a DigitalOcean Spaces bucket, through its S3-compatible
//! API. Public objects are served from the bucket's CDN.
//!
//! Configured from the environment: S3_BUCKET names the bucket, AWS_REGION is
//! the Spaces region (e.g. lon1), and AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY
//! are the Spaces access key, read by the AWS SDK itself.
use std::env;

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{Delete, ObjectCannedAcl, ObjectIdentifier};
use chrono::{DateTime, Utc};
use rand::distributions::Alphanumeric;
use rand::Rng;

/// Length of the random names given to uploaded objects
const RANDOM_NAME_LEN: usize = 12;
/// Most keys S3 deletes in one request
const MAX_DELETE_KEYS: usize = 1000;

/// An object listed in the bucket
#[derive(Debug)]
pub struct S3Object {
    pub key: String,
    pub size: i64,
    pub last_modified: Option<DateTime<Utc>>
}

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

    /// Every object whose key starts with `prefix`
    pub async fn list(&self, prefix: &str) -> Result<Vec<S3Object>, String> {
        let mut objects = Vec::new();
        let mut pages = self.client.list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .into_paginator()
            .send();
        while let Some(page) = pages.next().await {
            let page = page.map_err(|e| format!("Failed to list {prefix} in S3: {e}"))?;
            for object in page.contents() {
                let Some(key) = object.key() else { continue };
                objects.push(S3Object {
                    key: key.to_string(),
                    size: object.size().unwrap_or(0),
                    last_modified: object.last_modified()
                        .and_then(|t| DateTime::from_timestamp(t.secs(), t.subsec_nanos()))
                });
            }
        }
        Ok(objects)
    }

    pub async fn delete(&self, keys: &[String]) -> Result<(), String> {
        for chunk in keys.chunks(MAX_DELETE_KEYS) {
            let mut object_ids = Vec::with_capacity(chunk.len());
            for key in chunk {
                let object_id = ObjectIdentifier::builder()
                    .key(key)
                    .build()
                    .map_err(|e| format!("Failed to build key for delete_object: {e:?}"))?;
                object_ids.push(object_id);
            }
            let delete = Delete::builder()
                .set_objects(Some(object_ids))
                .build()
                .map_err(|e| format!("Failed to build delete_object input: {e:?}"))?;
            let output = self.client.delete_objects()
                .bucket(&self.bucket)
                .delete(delete)
                .send()
                .await
                .map_err(|e| format!("Failed to delete from S3: {e}"))?;
            // Each key succeeds or fails on its own
            if let Some(error) = output.errors().first() {
                return Err(format!("Failed to delete {} from S3: {}",
                    error.key().unwrap_or("?"), error.message().unwrap_or("unknown error")));
            }
        }
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
