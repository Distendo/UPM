use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::downloader::Downloader;
use crate::errors::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRepo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub default_branch: String,
    pub license: Option<GithubLicense>,
    #[serde(rename = "private")]
    pub is_private: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubLicense {
    pub spdx_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub assets: Vec<GithubAsset>,
    pub tarball_url: Option<String>,
    pub zipball_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: i64,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubContent {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub content_type: String,
    pub download_url: Option<String>,
}

pub struct GithubApi {
    client: Downloader,
    token: Option<String>,
    base_url: String,
}

impl GithubApi {
    pub fn new(token: Option<String>) -> Self {
        let client = Downloader::new(4);
        Self {
            client,
            token,
            base_url: "https://api.github.com".to_string(),
        }
    }

    fn headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Accept".to_string(), "application/vnd.github.v3+json".to_string());
        if let Some(ref token) = self.token {
            headers.insert("Authorization".to_string(), format!("Bearer {}", token));
        }
        headers
    }

    pub async fn search_repositories(&self, query: &str) -> Result<Vec<GithubRepo>> {
        let url = format!("{}/search/repositories?q={}&per_page=20", self.base_url, urlencoding(query));
        #[derive(Deserialize)]
        struct SearchResult {
            items: Vec<GithubRepo>,
        }
        let result: SearchResult = self.client.download_json(&url).await?;
        Ok(result.items)
    }

    pub async fn get_repository(&self, owner: &str, repo: &str) -> Result<GithubRepo> {
        let url = format!("{}/repos/{}/{}", self.base_url, owner, repo);
        self.client.download_json(&url).await
    }

    pub async fn get_releases(&self, owner: &str, repo: &str) -> Result<Vec<GithubRelease>> {
        let url = format!("{}/repos/{}/{}/releases?per_page=10", self.base_url, owner, repo);
        self.client.download_json(&url).await
    }

    pub async fn get_latest_release(&self, owner: &str, repo: &str) -> Result<GithubRelease> {
        let url = format!("{}/repos/{}/{}/releases/latest", self.base_url, owner, repo);
        self.client.download_json(&url).await
    }

    pub async fn get_repo_contents(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
    ) -> Result<Vec<GithubContent>> {
        let url = format!("{}/repos/{}/{}/contents/{}", self.base_url, owner, repo, path);
        self.client.download_json(&url).await
    }

    pub async fn download_release_asset(
        &self,
        url: &str,
        dest: &std::path::Path,
    ) -> Result<Vec<u8>> {
        self.client.download_file(url, dest).await
    }

    pub async fn download_tarball(
        &self,
        owner: &str,
        repo: &str,
        tag: &str,
        dest: &std::path::Path,
    ) -> Result<Vec<u8>> {
        let url = format!("https://api.github.com/repos/{}/{}/tarball/{}", owner, repo, tag);
        self.client.download_file(&url, dest).await
    }

    pub async fn download_raw_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> Result<Vec<u8>> {
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            owner, repo, branch, path
        );
        self.client.download_bytes(&url).await
    }

    pub fn client(&self) -> &Downloader {
        &self.client
    }
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}
