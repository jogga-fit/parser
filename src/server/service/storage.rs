//! S3-compatible object storage upload via AWS Signature Version 4.

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::server::{config::StorageConfig, error::AppError};

type HmacSha256 = Hmac<Sha256>;

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn sha256_hex(data: &[u8]) -> String {
    hex_encode(&Sha256::digest(data))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Upload `data` to S3-compatible storage and return the public URL.
///
/// Uses AWS Signature Version 4 when credentials are present. If either
/// `access_key_id` or `secret_access_key` is absent/empty, the PUT is sent
/// unsigned (useful for local MinIO with no auth).
pub async fn upload_bytes(
    config: &StorageConfig,
    key: &str,
    data: &[u8],
    content_type: &str,
) -> Result<String, AppError> {
    let region = config.region.as_deref().unwrap_or("us-east-1");
    let bucket = &config.bucket;

    // Build the PUT URL and Host header value.
    let (put_url, host) = if let Some(endpoint) = &config.endpoint {
        let endpoint = endpoint.trim_end_matches('/');
        let url = format!("{}/{}/{}", endpoint, bucket, key);
        // Extract just the host[:port] from the endpoint URL.
        let parsed = url::Url::parse(endpoint)
            .map_err(|e| AppError::Internal(crate::server::error::InternalError::UrlParse(e)))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| {
                AppError::Internal(crate::server::error::InternalError::Unexpected(
                    "endpoint has no host".to_string(),
                ))
            })?
            .to_string();
        let host = if let Some(port) = parsed.port() {
            format!("{}:{}", host, port)
        } else {
            host
        };
        (url, host)
    } else {
        let host = format!("{}.s3.{}.amazonaws.com", bucket, region);
        let url = format!("https://{}/{}", host, key);
        (url, host)
    };

    let now = Utc::now();
    let date_time = now.format("%Y%m%dT%H%M%SZ").to_string(); // e.g. 20240101T120000Z
    let date_only = &date_time[..8]; // e.g. 20240101

    let payload_hash = sha256_hex(data);

    // Headers (sorted alphabetically: content-type, host, x-amz-content-sha256, x-amz-date)
    let signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date";
    let canonical_headers = format!(
        "content-type:{}\nhost:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
        content_type, host, payload_hash, date_time
    );

    let canonical_request = format!(
        "PUT\n/{}\n\n{}\n{}\n{}",
        key, canonical_headers, signed_headers, payload_hash
    );

    let credential_scope = format!("{}/{}/s3/aws4_request", date_only, region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        date_time,
        credential_scope,
        sha256_hex(canonical_request.as_bytes())
    );

    let access_key = config
        .access_key_id
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();
    let secret_key = config
        .secret_access_key
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();

    let client = reqwest::Client::new();
    let mut request = client
        .put(&put_url)
        .header("content-type", content_type)
        .header("host", &host)
        .header("x-amz-content-sha256", &payload_hash)
        .header("x-amz-date", &date_time)
        .body(data.to_vec());

    if !access_key.is_empty() && !secret_key.is_empty() {
        // Derive signing key: HMAC(HMAC(HMAC(HMAC("AWS4"+secret, date), region), "s3"), "aws4_request")
        let signing_key = {
            let k_date = hmac_sha256(format!("AWS4{}", secret_key).as_bytes(), date_only.as_bytes());
            let k_region = hmac_sha256(&k_date, region.as_bytes());
            let k_service = hmac_sha256(&k_region, b"s3");
            hmac_sha256(&k_service, b"aws4_request")
        };

        let signature = hex_encode(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));

        let auth_header = format!(
            "AWS4-HMAC-SHA256 Credential={}/{},SignedHeaders={},Signature={}",
            access_key, credential_scope, signed_headers, signature
        );

        request = request.header("authorization", auth_header);
    }

    let response = request.send().await.map_err(|e| {
        AppError::Internal(crate::server::error::InternalError::Unexpected(
            format!("storage upload failed: {}", e),
        ))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Internal(
            crate::server::error::InternalError::Unexpected(format!(
                "storage upload returned {}: {}",
                status, body
            )),
        ));
    }

    let public_url = format!(
        "{}/{}",
        config.public_url.trim_end_matches('/'),
        key
    );
    Ok(public_url)
}
