use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::errors::{Result, UpmError};

pub struct Downloader {
    client: reqwest::Client,
    max_concurrency: usize,
}

impl Downloader {
    pub fn new(max_concurrency: usize) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("UPM/0.1.0")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap_or_default();

        Self {
            client,
            max_concurrency,
        }
    }

    pub async fn download_file(&self, url: &str, dest: &Path) -> Result<Vec<u8>> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let response = self
            .client
            .get(url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(UpmError::Network(format!(
                "HTTP {} for {}",
                response.status(),
                url
            )));
        }

        let total_size = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

        let mut data = Vec::with_capacity(total_size as usize);
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            data.extend_from_slice(&chunk);
            pb.inc(chunk.len() as u64);
        }

        pb.finish_and_clear();

        std::fs::write(dest, &data)?;
        Ok(data)
    }

    pub async fn download_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(UpmError::Network(format!(
                "HTTP {} for {}",
                response.status(),
                url
            )));
        }

        let data = response
            .bytes()
            .await?;

        Ok(data.to_vec())
    }

    pub async fn download_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<T> {
        let response = self
            .client
            .get(url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(UpmError::Network(format!(
                "HTTP {} for {}",
                response.status(),
                url
            )));
        }

        let data: T = response
            .json()
            .await?;

        Ok(data)
    }

    pub async fn download_concurrent(
        self: &Arc<Self>,
        urls: Vec<(String, PathBuf)>,
    ) -> Vec<Result<Vec<u8>>> {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_concurrency));
        let mut handles = Vec::new();

        for (url, dest) in urls {
            let sem = Arc::clone(&semaphore);
            let this = Arc::clone(self);
            let handle = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                this.download_file(&url, &dest).await
            });
            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(UpmError::General(format!("Task failed: {e}")))),
            }
        }

        results
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
}
