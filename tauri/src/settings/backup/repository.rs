//! GitHub/Gitee repository storage adapter for backup files.
//!
//! Reuses the auth/URL/error handling proven in the retired configuration-sync
//! client, but stores whole backup files instead of projected configuration
//! snapshots. Platform semantics are kept separate:
//! - create: GitHub Contents PUT, Gitee Contents POST (no sha → a same-name file
//!   can never be silently overwritten);
//! - delete: GitHub DELETE with a JSON body, Gitee DELETE with query parameters;
//! - download: metadata first, verify the listed SHA, then read the blob.

use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, Method, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::credentials::backup_error;
use super::filename::{backup_sort_key, is_managed_backup_filename};
use super::repository_settings::{
    load_backup_repository_settings, normalize, resolve_repository_token, BackupRepositoryConfig,
    BackupRepositorySettings,
};
use crate::db::SqliteDbState;
use crate::http_client;

/// GitHub refuses Contents API operations on files above 100 MB; uploads and
/// downloads both pre-check this platform-specific limit. Gitee has no documented
/// per-file API limit, so its errors are passed through untouched.
const GITHUB_MAX_FILE_BYTES: usize = 100 * 1024 * 1024;
/// GitHub Contents directory listings are silently truncated at 1000 entries; that
/// count (or more) re-reads the directory via the recursive Git Trees API.
const GITHUB_CONTENTS_TRUNCATION: usize = 1000;
/// Gitee Contents has no pagination at all — page/per_page are ignored (verified
/// against the official OpenAPI schema and a live public repository) and large
/// directories are capped. At this count the listing may already be incomplete, so
/// it re-reads via the recursive Git Trees API.
const GITEE_CONTENTS_SUSPECTED_CAP: usize = 100;
/// Safety net for response bodies so a misbehaving server cannot exhaust memory;
/// far above any realistic backup but not a documented platform limit.
const MAX_DOWNLOAD_BYTES: usize = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteBackupFileInfo {
    pub filename: String,
    pub size: u64,
    pub sha: String,
    pub encrypted: bool,
}

pub struct RepositoryClient {
    client: Client,
    platform: super::repository_settings::BackupRepositoryPlatform,
    owner: String,
    repository: String,
    branch: String,
    directory: String,
    token: String,
    api_origin: String,
}

impl RepositoryClient {
    pub fn new(client: Client, settings: &BackupRepositorySettings) -> Result<Self, String> {
        validate_config(&settings.config)?;
        if settings.token.is_empty() {
            return Err(backup_error(
                "tokenRequired",
                "settings.backupSettings.repository.errors.tokenRequired",
                "repository token is required",
            ));
        }
        let platform = settings.config.platform;
        Ok(Self {
            client,
            platform,
            owner: settings.config.owner.clone(),
            repository: settings.config.repository.clone(),
            branch: settings.config.branch.clone(),
            directory: settings.config.directory.clone(),
            token: settings.token.clone(),
            api_origin: match platform {
                super::repository_settings::BackupRepositoryPlatform::Github => {
                    "https://api.github.com"
                }
                super::repository_settings::BackupRepositoryPlatform::Gitee => {
                    "https://gitee.com/api/v5"
                }
            }
            .into(),
        })
    }

    /// Test-only constructor that skips credential validation so mock-server tests
    /// can point the client at a local listener.
    #[cfg(test)]
    pub(crate) fn for_test(
        client: Client,
        settings: &BackupRepositorySettings,
        api_origin: String,
    ) -> Self {
        Self {
            client,
            platform: settings.config.platform,
            owner: settings.config.owner.clone(),
            repository: settings.config.repository.clone(),
            branch: settings.config.branch.clone(),
            directory: settings.config.directory.clone(),
            token: settings.token.clone(),
            api_origin,
        }
    }

    fn endpoint(&self, suffix: &[&str]) -> Result<url::Url, String> {
        let mut url = url::Url::parse(&self.api_origin).map_err(|error| error.to_string())?;
        let mut path = url
            .path_segments_mut()
            .map_err(|_| "Invalid API URL".to_string())?;
        path.pop_if_empty()
            .push("repos")
            .push(&self.owner)
            .push(&self.repository);
        for part in suffix {
            path.push(part);
        }
        drop(path);
        Ok(url)
    }

    fn request(&self, method: Method, url: url::Url) -> RequestBuilder {
        let request = self
            .client
            .request(method, url)
            .header("User-Agent", "AI-Toolbox-Backup");
        match self.platform {
            super::repository_settings::BackupRepositoryPlatform::Github => request
                .bearer_auth(&self.token)
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28"),
            // Never log this URL: it carries the access token as a query parameter.
            super::repository_settings::BackupRepositoryPlatform::Gitee => {
                request.query(&[("access_token", &self.token)])
            }
        }
    }

    async fn send(&self, request: RequestBuilder) -> Result<reqwest::Response, String> {
        request.send().await.map_err(|_| {
            backup_error(
                "network",
                "settings.backupSettings.repository.errors.network",
                "repository request failed",
            )
        })
    }

    /// Read-only reachability check for repository, branch, and directory.
    /// A missing directory is tolerated: Contents APIs create it on first upload.
    pub async fn check_repository(&self) -> Result<(), String> {
        self.ensure_private_repository().await?;
        let response = self
            .send(self.request(Method::GET, self.endpoint(&["branches", &self.branch])?))
            .await?;
        check_status(response.status())
    }

    /// Private-repository enforcement for every real write. The same rule the
    /// connection test applies, but invoked from `upload_file` so skipping the test
    /// — or a repository flipped to public afterwards — can never put an unencrypted
    /// full backup (API keys, OAuth state, …) on a public repository.
    async fn ensure_private_repository(&self) -> Result<(), String> {
        let response = self
            .send(self.request(Method::GET, self.endpoint(&[])?))
            .await?;
        check_status(response.status())?;
        let repository: Value = response
            .json()
            .await
            .map_err(|_| repository_response_error())?;
        if repository.get("private").and_then(Value::as_bool) != Some(true) {
            return Err(backup_error(
                "privateRepository",
                "settings.backupSettings.repository.errors.privateRepository",
                "repository must be private",
            ));
        }
        Ok(())
    }

    fn contents_path(&self, filename: &str) -> Vec<String> {
        let mut suffix: Vec<String> = ["contents"].into_iter().map(str::to_string).collect();
        suffix.extend(
            self.directory
                .split('/')
                .filter(|part| !part.is_empty())
                .map(str::to_string),
        );
        suffix.push(filename.to_string());
        suffix
    }

    fn contents_url(&self, filename: &str) -> Result<url::Url, String> {
        let path_parts = self.contents_path(filename);
        let parts: Vec<&str> = path_parts.iter().map(String::as_str).collect();
        self.endpoint(&parts)
    }

    fn directory_url(&self) -> Result<url::Url, String> {
        let mut parts: Vec<&str> = vec!["contents"];
        let segments: Vec<&str> = self
            .directory
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        if segments.is_empty() {
            // Repository root; the endpoint ends at the repo.
            return self.endpoint(&parts);
        }
        parts.extend(segments);
        self.endpoint(&parts)
    }

    /// List managed backup files in the configured directory. Only files matching
    /// the shared backup filename contract are returned, so an old `.aitsync`
    /// snapshot or user files in the same directory are never offered or cleaned up.
    pub async fn list_backups(&self) -> Result<Vec<RemoteBackupFileInfo>, String> {
        Ok(self.list_backups_detailed().await?.backups)
    }

    /// List managed backups plus a completeness flag. Retention cleanup must refuse
    /// to delete anything when the underlying platform listing was truncated.
    pub async fn list_backups_detailed(&self) -> Result<RemoteBackupListing, String> {
        let mut entries = self.list_directory().await?;
        let mut truncated = false;
        // Contents listings cannot always be trusted for completeness: GitHub
        // silently truncates at 1000 entries and Gitee has no pagination at all
        // (its API ignores page/per_page and caps large directories), so a large
        // directory re-reads via the recursive Git Trees API, whose `truncated`
        // flag is the only completeness signal either platform offers.
        let needs_trees = match self.platform {
            super::repository_settings::BackupRepositoryPlatform::Github => {
                entries.len() >= GITHUB_CONTENTS_TRUNCATION
            }
            super::repository_settings::BackupRepositoryPlatform::Gitee => {
                entries.len() >= GITEE_CONTENTS_SUSPECTED_CAP
            }
        };
        if needs_trees {
            let tree = self.list_directory_via_git_trees().await?;
            truncated = tree.truncated;
            entries = tree.entries;
        }
        let mut backups: Vec<RemoteBackupFileInfo> = entries
            .into_iter()
            .filter(|entry| entry.entry_type == "file" && is_managed_backup_filename(&entry.name))
            .map(|entry| RemoteBackupFileInfo {
                encrypted: entry.name.ends_with(".zip.enc"),
                filename: entry.name,
                size: entry.size,
                sha: entry.sha,
            })
            .collect();
        backups.sort_by(|a, b| backup_sort_key(&b.filename).cmp(&backup_sort_key(&a.filename)));
        Ok(RemoteBackupListing {
            backups,
            complete: !truncated,
        })
    }

    async fn list_directory(&self) -> Result<Vec<RemoteDirectoryEntry>, String> {
        // Single Contents request: GitHub returns the whole directory (up to its
        // 1000-entry truncation, re-read via trees in list_backups_detailed) and
        // Gitee ignores pagination parameters entirely, so looping would re-fetch
        // the same batch forever.
        let request = self
            .request(Method::GET, self.directory_url()?)
            .query(&[("ref", &self.branch)]);
        let response = self.send(request).await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        check_status(response.status())?;
        let items: Value = response
            .json()
            .await
            .map_err(|_| repository_response_error())?;
        parse_directory_entries(&items)
    }

    async fn list_directory_via_git_trees(&self) -> Result<RemoteTreeListing, String> {
        // Prefer resolving the branch head to a commit sha: both platforms document
        // the trees endpoint around a sha, and the branches endpoint above is
        // already proven for both.
        let commit_sha = self.resolve_branch_commit_sha().await?;
        let response = self
            .send(
                self.request(Method::GET, self.endpoint(&["git", "trees", &commit_sha])?)
                    .query(&[("recursive", "1")]),
            )
            .await?;
        check_status(response.status())?;
        let tree: Value = response
            .json()
            .await
            .map_err(|_| repository_response_error())?;
        let mut listing = parse_tree_listing(&tree)?;
        let prefix = {
            let mut prefix = self
                .directory
                .split('/')
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("/");
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix
        };
        listing.entries.retain(|entry| {
            entry.path.starts_with(&prefix) && !entry.path[prefix.len()..].contains('/')
        });
        for entry in &mut listing.entries {
            entry.name = entry.path[prefix.len()..].to_string();
            // Git Trees labels files "blob"; normalize to the Contents API spelling.
            if entry.entry_type == "blob" {
                entry.entry_type = "file".to_string();
            }
        }
        Ok(listing)
    }

    async fn resolve_branch_commit_sha(&self) -> Result<String, String> {
        let branch_response = self
            .send(self.request(Method::GET, self.endpoint(&["branches", &self.branch])?))
            .await?;
        check_status(branch_response.status())?;
        let branch: Value = branch_response
            .json()
            .await
            .map_err(|_| repository_response_error())?;
        branch
            .pointer("/commit/sha")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(repository_response_error)
    }

    /// Read one file, verifying the listed SHA so a file replaced after listing is
    /// never restored by mistake. Small files come from Contents; larger ones from
    /// the Git Blob endpoint.
    pub async fn read_file(
        &self,
        filename: &str,
        expected_sha: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        if !is_managed_backup_filename(filename) {
            return Err(backup_error(
                "invalidFilename",
                "settings.backupSettings.repository.errors.invalidFilename",
                "not a managed backup filename",
            ));
        }
        let mut request = self
            .request(Method::GET, self.contents_url(filename)?)
            .query(&[("ref", &self.branch)]);
        if self.platform == super::repository_settings::BackupRepositoryPlatform::Github {
            request = request.header("Accept", "application/vnd.github.object+json");
        }
        let response = self.send(request).await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(backup_error(
                "notFound",
                "settings.backupSettings.repository.errors.notFound",
                "backup file no longer exists",
            ));
        }
        check_status(response.status())?;
        let metadata: Value =
            serde_json::from_slice(&read_limited(response, MAX_DOWNLOAD_BYTES).await?)
                .map_err(|_| repository_response_error())?;
        let sha = metadata
            .get("sha")
            .and_then(Value::as_str)
            .filter(|sha| sha.len() <= 128 && sha.chars().all(|c| c.is_ascii_hexdigit()))
            .ok_or_else(repository_response_error)?
            .to_string();
        // Only verify when the caller actually has a listed SHA; an empty one means
        // "no expectation" and must not fail the comparison.
        if let Some(expected) = expected_sha.filter(|expected| !expected.is_empty()) {
            if expected != sha {
                return Err(backup_error(
                    "remoteChanged",
                    "settings.backupSettings.repository.errors.remoteChanged",
                    "backup file changed after listing",
                ));
            }
        }
        let size = metadata.get("size").and_then(Value::as_u64).unwrap_or(0);
        if self.platform == super::repository_settings::BackupRepositoryPlatform::Github
            && size as usize > GITHUB_MAX_FILE_BYTES
        {
            return Err(self.too_large_error());
        }
        if let Some(content) = metadata
            .get("content")
            .and_then(Value::as_str)
            .filter(|content| !content.is_empty())
        {
            return decode_base64(content);
        }
        // Larger files: read the blob by SHA.
        let response = self
            .send(self.request(Method::GET, self.endpoint(&["git", "blobs", &sha])?))
            .await?;
        check_status(response.status())?;
        let blob = read_limited(response, MAX_DOWNLOAD_BYTES).await?;
        let blob: Value = serde_json::from_slice(&blob).map_err(|_| repository_response_error())?;
        decode_base64(
            blob.get("content")
                .and_then(Value::as_str)
                .ok_or_else(repository_response_error)?,
        )
    }

    /// Upload a finished backup file. The private-repository rule is enforced here
    /// (one read-only repo request) so every real write — manual and auto backup
    /// alike — refuses public repositories regardless of whether the user ever ran
    /// the connection test. Creating (never updating) semantics are enforced by the
    /// API: GitHub PUT without a sha and Gitee POST both refuse existing files, so
    /// an old backup with the same name is kept.
    pub async fn upload_file(&self, filename: &str, bytes: &[u8]) -> Result<(), String> {
        self.ensure_private_repository().await?;
        if !is_managed_backup_filename(filename) {
            return Err(backup_error(
                "invalidFilename",
                "settings.backupSettings.repository.errors.invalidFilename",
                "not a managed backup filename",
            ));
        }
        if self.platform == super::repository_settings::BackupRepositoryPlatform::Github
            && bytes.len() > GITHUB_MAX_FILE_BYTES
        {
            return Err(self.too_large_error());
        }
        let method = match self.platform {
            super::repository_settings::BackupRepositoryPlatform::Github => Method::PUT,
            super::repository_settings::BackupRepositoryPlatform::Gitee => Method::POST,
        };
        let body = json!({
            "message": format!("AI Toolbox backup: {filename}"),
            "branch": self.branch,
            "content": STANDARD.encode(bytes),
        });
        let response = self
            .send(
                self.request(method, self.contents_url(filename)?)
                    .json(&body),
            )
            .await?;
        if matches!(response.status().as_u16(), 409 | 422) {
            return Err(backup_error(
                "fileExists",
                "settings.backupSettings.repository.errors.fileExists",
                "a backup with the same name already exists; it was kept untouched",
            ));
        }
        check_status(response.status())
    }

    /// Delete the file bound to its listed SHA.
    pub async fn delete_file(&self, filename: &str, sha: &str) -> Result<(), String> {
        if !is_managed_backup_filename(filename) {
            return Err(backup_error(
                "invalidFilename",
                "settings.backupSettings.repository.errors.invalidFilename",
                "not a managed backup filename",
            ));
        }
        if sha.is_empty() {
            return Err(backup_error(
                "remoteResponse",
                "settings.backupSettings.repository.errors.remoteResponse",
                "missing file sha for delete",
            ));
        }
        let response = match self.platform {
            super::repository_settings::BackupRepositoryPlatform::Github => {
                let body = json!({
                    "message": format!("AI Toolbox backup cleanup: {filename}"),
                    "branch": self.branch,
                    "sha": sha,
                });
                self.send(
                    self.request(Method::DELETE, self.contents_url(filename)?)
                        .json(&body),
                )
                .await?
            }
            super::repository_settings::BackupRepositoryPlatform::Gitee => {
                self.send(
                    self.request(Method::DELETE, self.contents_url(filename)?)
                        .query(&[("sha", sha)])
                        .query(&[("branch", &self.branch)])
                        .query(&[(
                            "message",
                            format!("AI Toolbox backup cleanup: {filename}").as_str(),
                        )]),
                )
                .await?
            }
        };
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            // Already gone: treat as success so retention cleanup can proceed.
            return Ok(());
        }
        check_status(response.status())
    }

    fn too_large_error(&self) -> String {
        backup_error(
            "tooLarge",
            "settings.backupSettings.repository.errors.tooLarge",
            "backup file exceeds the GitHub Contents API 100 MB limit",
        )
    }
}

pub async fn repository_client_from_settings(
    db_state: &SqliteDbState,
    settings: &BackupRepositorySettings,
) -> Result<RepositoryClient, String> {
    let client = http_client::client_with_timeout(db_state, 300).await?;
    RepositoryClient::new(client, settings)
}

#[derive(Debug, Clone)]
struct RemoteDirectoryEntry {
    name: String,
    path: String,
    size: u64,
    sha: String,
    entry_type: String,
}

/// Listing result with a completeness flag. `complete: false` means the platform
/// reported a truncated listing; retention cleanup must not delete from it.
#[derive(Debug, Clone)]
pub struct RemoteBackupListing {
    pub backups: Vec<RemoteBackupFileInfo>,
    pub complete: bool,
}

/// Parse a recursive Git Trees response. GitHub and Gitee share the shape: entries
/// carry path/type/mode/sha/size and never a `name` field, so the file name is the
/// last path segment. `truncated` is the only completeness signal either platform
/// offers on this endpoint.
fn parse_tree_listing(value: &Value) -> Result<RemoteTreeListing, String> {
    let items = value
        .get("tree")
        .and_then(Value::as_array)
        .ok_or_else(repository_response_error)?;
    let mut entries = Vec::new();
    for item in items {
        let path = item
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(repository_response_error)?
            .to_string();
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        entries.push(RemoteDirectoryEntry {
            name,
            path,
            size: item.get("size").and_then(Value::as_u64).unwrap_or(0),
            sha: item
                .get("sha")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            entry_type: item
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("file")
                .to_string(),
        });
    }
    Ok(RemoteTreeListing {
        entries,
        truncated: value
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

#[derive(Debug, Clone)]
struct RemoteTreeListing {
    entries: Vec<RemoteDirectoryEntry>,
    truncated: bool,
}

fn parse_directory_entries(value: &Value) -> Result<Vec<RemoteDirectoryEntry>, String> {
    let items = value.as_array().ok_or_else(repository_response_error)?;
    let mut entries = Vec::new();
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(repository_response_error)?
            .to_string();
        let path = item
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or(&name)
            .to_string();
        entries.push(RemoteDirectoryEntry {
            name,
            path,
            size: item.get("size").and_then(Value::as_u64).unwrap_or(0),
            sha: item
                .get("sha")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            entry_type: item
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("file")
                .to_string(),
        });
    }
    Ok(entries)
}

async fn read_limited(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(backup_error(
            "tooLarge",
            "settings.backupSettings.repository.errors.fileTooLarge",
            "response body exceeds the safety limit",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        backup_error(
            "network",
            "settings.backupSettings.repository.errors.network",
            "repository request failed",
        )
    })? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(backup_error(
                "tooLarge",
                "settings.backupSettings.repository.errors.fileTooLarge",
                "response body exceeds the safety limit",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn decode_base64(content: &str) -> Result<Vec<u8>, String> {
    STANDARD
        .decode(
            content
                .chars()
                .filter(|character| !character.is_ascii_whitespace())
                .collect::<String>(),
        )
        .map_err(|_| repository_response_error())
}

fn repository_response_error() -> String {
    backup_error(
        "remoteResponse",
        "settings.backupSettings.repository.errors.remoteResponse",
        "unexpected repository API response",
    )
}

fn check_status(status: reqwest::StatusCode) -> Result<(), String> {
    if status.is_success() {
        return Ok(());
    }
    Err(match status.as_u16() {
        401 | 403 => backup_error(
            "permission",
            "settings.backupSettings.repository.errors.permission",
            "repository access denied",
        ),
        404 => backup_error(
            "repositoryNotFound",
            "settings.backupSettings.repository.errors.repositoryNotFound",
            "repository or branch not found",
        ),
        413 => backup_error(
            "tooLarge",
            "settings.backupSettings.repository.errors.fileTooLarge",
            "request body too large for the platform",
        ),
        429 => backup_error(
            "rateLimit",
            "settings.backupSettings.repository.errors.rateLimit",
            "repository API rate limit reached",
        ),
        _ => backup_error(
            "remoteResponse",
            "settings.backupSettings.repository.errors.remoteResponse",
            &format!("repository API status: {status}"),
        ),
    })
}

/// Validate the repository connection draft. Owner/repo must be plain GitHub-style
/// names; the directory must be a safe relative path without escaping segments.
pub fn validate_config(
    config: &super::repository_settings::BackupRepositoryConfig,
) -> Result<(), String> {
    for value in [&config.owner, &config.repository] {
        if value.is_empty()
            || value == "."
            || value == ".."
            || !value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
        {
            return Err(backup_error(
                "invalidRepository",
                "settings.backupSettings.repository.errors.invalidRepository",
                "owner and repository must be plain names",
            ));
        }
    }
    if config.branch.is_empty() || config.branch.chars().any(char::is_control) {
        return Err(backup_error(
            "invalidBranch",
            "settings.backupSettings.repository.errors.invalidBranch",
            "branch name is invalid",
        ));
    }
    if !config.directory.is_empty()
        && (!valid_relative_path(&config.directory)
            || config
                .directory
                .split('/')
                .any(|part| part == ".git" || part == ".github"))
    {
        return Err(backup_error(
            "invalidDirectory",
            "settings.backupSettings.repository.errors.invalidDirectory",
            "directory must be a safe relative path",
        ));
    }
    Ok(())
}

fn valid_relative_path(path: &str) -> bool {
    !path.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        && !path.chars().any(char::is_control)
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

async fn repository_client(
    db_state: &SqliteDbState,
    config: BackupRepositoryConfig,
    token: Option<String>,
) -> Result<RepositoryClient, String> {
    let mut settings = load_backup_repository_settings(db_state)?;
    let stored = settings.clone();
    // A fully/un- submitted draft falls back to the stored connection; a partially
    // filled draft replaces it for this read-only check.
    if !config.is_unconfigured() {
        settings.config = normalize(config);
    }
    // Credential selection shared with the save path: an empty draft token only
    // reuses the stored token on the same platform, so a GitHub token can never be
    // sent to Gitee (or vice versa) by a platform switch + connection test.
    settings.token = resolve_repository_token(&stored, &settings.config, token)?;
    let client = http_client::client_with_timeout(db_state, 300).await?;
    RepositoryClient::new(client, &settings)
}

/// Test repository reachability using the submitted draft (empty token falls back
/// to the stored credential). Read-only: repository, branch, and directory checks.
#[tauri::command]
pub async fn test_backup_repository_connection(
    db: tauri::State<'_, SqliteDbState>,
    config: BackupRepositoryConfig,
    token: Option<String>,
) -> Result<(), String> {
    let client = repository_client(&db, config, token).await?;
    client.check_repository().await
}

/// List managed backup files stored in the configured repository directory.
#[tauri::command]
pub async fn list_repository_backups(
    db: tauri::State<'_, SqliteDbState>,
) -> Result<Vec<RemoteBackupFileInfo>, String> {
    let settings = load_backup_repository_settings(&db)?;
    let client = repository_client_from_settings(&db, &settings).await?;
    client.list_backups().await
}

/// Backup to the configured GitHub/Gitee repository via the shared generation layer.
#[tauri::command]
pub async fn backup_to_repository(
    app_handle: tauri::AppHandle,
    db: tauri::State<'_, SqliteDbState>,
) -> Result<String, String> {
    let settings = load_backup_repository_settings(&db)?;
    let client = repository_client_from_settings(&db, &settings).await?;
    let generated = super::generate::generate_backup_file(&app_handle, None).await?;
    client
        .upload_file(&generated.filename, &generated.bytes)
        .await?;
    Ok(generated.filename)
}

/// Restore a repository backup. Downloads by the listed SHA, decrypts when needed,
/// then runs the shared restore pipeline.
#[tauri::command]
pub async fn restore_from_repository(
    app_handle: tauri::AppHandle,
    db: tauri::State<'_, SqliteDbState>,
    filename: String,
    sha: String,
    skip_cli_custom_roots: Option<bool>,
    restore_password: Option<String>,
) -> Result<super::utils::RestoreResult, String> {
    let skip_cli_custom_roots = skip_cli_custom_roots.unwrap_or(false);
    let settings = load_backup_repository_settings(&db)?;
    let client = repository_client_from_settings(&db, &settings).await?;
    let bytes = client.read_file(&filename, Some(&sha)).await?;
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        super::restore::prepare_backup_bytes(bytes, restore_password.as_deref())
    })
    .await
    .map_err(|error| error.to_string())??;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("Failed to read zip archive: {}", e))?;
    super::restore::restore_from_archive(&app_handle, &mut archive, skip_cli_custom_roots)
}

/// Delete one repository backup bound to its listed SHA.
#[tauri::command]
pub async fn delete_repository_backup(
    db: tauri::State<'_, SqliteDbState>,
    filename: String,
    sha: String,
) -> Result<(), String> {
    let settings = load_backup_repository_settings(&db)?;
    let client = repository_client_from_settings(&db, &settings).await?;
    client.delete_file(&filename, &sha).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_config_accepts_plain_names_and_rejects_traversal() {
        let mut config = super::super::repository_settings::BackupRepositoryConfig::default();
        config.owner = "someone".into();
        config.repository = "backups".into();
        config.branch = "main".into();
        config.directory = "ai-toolbox/backups".into();
        assert!(validate_config(&config).is_ok());

        config.directory = "../escape".into();
        assert!(validate_config(&config).is_err());

        config.directory = "ok/../../up".into();
        assert!(validate_config(&config).is_err());

        config.directory = "trailing/".into();
        assert!(
            validate_config(&config).is_err(),
            "empty path segment must be rejected"
        );

        config.owner = "bad/name".into();
        config.directory = "ok".into();
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn valid_relative_path_rejects_windows_and_absolute_forms() {
        assert!(valid_relative_path("a/b/c"));
        assert!(!valid_relative_path("/abs"));
        assert!(!valid_relative_path("C:\\x"));
        assert!(!valid_relative_path("a//b"));
        assert!(!valid_relative_path(""));
    }

    #[test]
    fn tree_listing_parses_official_shape_without_name_field() {
        // GitHub and Gitee tree entries carry path/type/sha/size but no `name`;
        // the file name must be derived from the last path segment.
        let listing = parse_tree_listing(&serde_json::json!({
            "sha": "treesha",
            "tree": [
                {"path": "backups/ai-toolbox-backup-20260913-120000-abc123ef.zip", "mode": "100644", "type": "blob", "sha": "blob1", "size": 1024},
                {"path": "backups/sub/inner.bin", "mode": "100644", "type": "blob", "sha": "blob2", "size": 2},
                {"path": "backups", "mode": "040000", "type": "tree", "sha": "dirsha"},
            ],
            "truncated": true,
        }))
        .expect("parse tree listing");
        assert!(listing.truncated);
        assert_eq!(listing.entries.len(), 3);
        assert_eq!(
            listing.entries[0].name,
            "ai-toolbox-backup-20260913-120000-abc123ef.zip"
        );
        assert_eq!(listing.entries[0].sha, "blob1");
        assert_eq!(listing.entries[0].size, 1024);
        assert_eq!(listing.entries[0].entry_type, "blob");
        assert_eq!(listing.entries[1].name, "inner.bin");
        assert_eq!(listing.entries[2].entry_type, "tree");
    }

    #[test]
    fn tree_listing_without_truncated_flag_counts_as_complete() {
        let listing = parse_tree_listing(&serde_json::json!({
            "tree": [{"path": "backups/a.zip", "type": "blob", "sha": "s", "size": 1}],
        }))
        .expect("parse tree listing");
        assert!(!listing.truncated);
    }

    #[test]
    fn tree_listing_rejects_missing_tree_array() {
        assert!(parse_tree_listing(&serde_json::json!({"items": []})).is_err());
        assert!(parse_tree_listing(&serde_json::json!({"tree": [{"mode": "100644"}]})).is_err());
    }
}

#[cfg(test)]
pub(crate) mod mock_api_tests {
    use super::*;
    use crate::settings::backup::repository_settings::BackupRepositoryPlatform;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    pub(crate) struct MockApi {
        pub(crate) url: String,
        requests: Arc<Mutex<Vec<String>>>,
    }

    /// Minimal sequential HTTP mock: serves the queued (status, body) responses in
    /// order and records "METHOD path" per request. The query is stripped from the
    /// log because Gitee requests carry the access token there.
    pub(crate) fn start_mock(responses: Vec<(u16, String)>) -> MockApi {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let request_log = requests.clone();
        std::thread::spawn(move || {
            let mut next = 0usize;
            for stream in listener.incoming().flatten() {
                if serve_request(stream, &responses, &mut next, &request_log).is_err() {
                    break;
                }
            }
        });
        MockApi { url, requests }
    }

    fn serve_request(
        mut stream: std::net::TcpStream,
        responses: &[(u16, String)],
        next: &mut usize,
        request_log: &Arc<Mutex<Vec<String>>>,
    ) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;
        // Consume headers and body so the socket is drained before responding.
        let mut content_length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
        if content_length > 0 {
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body)?;
        }

        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let target = parts.next().unwrap_or("");
        let path = target.split('?').next().unwrap_or("").to_string();
        request_log
            .lock()
            .expect("request log")
            .push(format!("{method} {path}"));

        let (status, body) = responses
            .get(*next)
            .cloned()
            .unwrap_or((500, "{\"message\":\"mock exhausted\"}".to_string()));
        *next += 1;
        let reason = match status {
            200 => "OK",
            404 => "Not Found",
            _ => "Error",
        };
        write!(
            stream,
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        )?;
        stream.flush()
    }

    pub(crate) fn test_client(
        server: &MockApi,
        platform: BackupRepositoryPlatform,
    ) -> RepositoryClient {
        RepositoryClient {
            client: reqwest::Client::new(),
            platform,
            owner: "owner".into(),
            repository: "repo".into(),
            branch: "main".into(),
            directory: "backups".into(),
            token: "test-token".into(),
            api_origin: server.url.clone(),
        }
    }

    pub(crate) fn recorded_requests(server: &MockApi) -> Vec<String> {
        server.requests.lock().expect("request log").clone()
    }

    #[tokio::test]
    async fn server_size_rejections_do_not_claim_a_github_specific_limit() {
        for platform in [
            BackupRepositoryPlatform::Github,
            BackupRepositoryPlatform::Gitee,
        ] {
            let server = start_mock(vec![
                (200, r#"{"private":true}"#.to_string()),
                (413, r#"{"message":"request body too large"}"#.to_string()),
            ]);
            let client = test_client(&server, platform);
            let error = client
                .upload_file("ai-toolbox-backup-20260913-120000-abc123ef.zip", b"fixture")
                .await
                .expect_err("the platform rejected the payload");
            let parsed: Value = serde_json::from_str(&error).unwrap();
            assert_eq!(parsed["type"], "tooLarge");
            assert_eq!(
                parsed["suggestion"],
                "settings.backupSettings.repository.errors.fileTooLarge"
            );
            assert!(!error.contains("100 MB"));
            assert!(!error.contains("GitHub"));
            assert_eq!(recorded_requests(&server).len(), 2);
        }
    }

    #[tokio::test]
    async fn public_repository_upload_is_refused_without_any_write_request() {
        let server = start_mock(vec![(
            200,
            r#"{"private": false, "name": "repo"}"#.to_string(),
        )]);
        let client = test_client(&server, BackupRepositoryPlatform::Github);

        let error = client
            .upload_file("ai-toolbox-backup-20260913-120000-abc123ef.zip", b"payload")
            .await
            .expect_err("public repository upload must be refused");
        assert!(
            error.contains("privateRepository"),
            "unexpected error: {error}"
        );

        // Only the read-only private check happened; no PUT reached the API.
        assert_eq!(
            recorded_requests(&server),
            vec!["GET /repos/owner/repo".to_string()]
        );
    }

    #[tokio::test]
    async fn private_repository_upload_proceeds() {
        let server = start_mock(vec![
            (200, r#"{"private": true}"#.to_string()),
            (201, r#"{"content": {}}"#.to_string()),
        ]);
        let client = test_client(&server, BackupRepositoryPlatform::Github);

        client
            .upload_file("ai-toolbox-backup-20260913-120000-abc123ef.zip", b"payload")
            .await
            .expect("private repository upload must succeed");
        assert_eq!(
            recorded_requests(&server),
            vec![
                "GET /repos/owner/repo".to_string(),
                "PUT /repos/owner/repo/contents/backups/ai-toolbox-backup-20260913-120000-abc123ef.zip"
                    .to_string(),
            ]
        );
    }

    pub(crate) fn contents_entries(count: usize) -> String {
        // Valid shared-contract timestamps: hour 12, minute index/60 (0 or 1),
        // second index%60 — unique names for up to 120 entries.
        let items: Vec<String> = (0..count)
            .map(|index| {
                let name = format!(
                    "ai-toolbox-backup-20260913-12{:02}{:02}.zip",
                    index / 60,
                    index % 60
                );
                format!(
                    r#"{{"name": "{name}", "path": "backups/{name}", "sha": "sha{index:03}", "size": 1, "type": "file"}}"#
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    #[tokio::test]
    async fn gitee_large_directory_falls_back_to_trees_and_stays_complete() {
        // Contents answers with the suspected cap (pagination ignored), so the
        // listing must re-read via trees instead of looping on page parameters.
        let server = start_mock(vec![
            (200, contents_entries(GITEE_CONTENTS_SUSPECTED_CAP)),
            (200, r#"{"commit": {"sha": "commitsha"}}"#.to_string()),
            (
                200,
                r#"{"sha": "treesha", "truncated": false, "tree": [
                    {"path": "backups/ai-toolbox-backup-20260913-120000-abc123ef.zip", "type": "blob", "sha": "blob1", "size": 5},
                    {"path": "backups/sub/inner.bin", "type": "blob", "sha": "blob2", "size": 2}
                ]}"#
                .to_string(),
            ),
        ]);
        let client = test_client(&server, BackupRepositoryPlatform::Gitee);

        let listing = client.list_backups_detailed().await.expect("listing");
        assert!(listing.complete);
        // The tree listing replaced the possibly-capped Contents result.
        assert_eq!(listing.backups.len(), 1);
        assert_eq!(
            listing.backups[0].filename,
            "ai-toolbox-backup-20260913-120000-abc123ef.zip"
        );
        assert_eq!(listing.backups[0].sha, "blob1");
        assert_eq!(
            recorded_requests(&server),
            vec![
                "GET /repos/owner/repo/contents/backups".to_string(),
                "GET /repos/owner/repo/branches/main".to_string(),
                "GET /repos/owner/repo/git/trees/commitsha".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn small_gitee_directory_uses_contents_only() {
        let server = start_mock(vec![(200, contents_entries(2))]);
        let client = test_client(&server, BackupRepositoryPlatform::Gitee);

        let listing = client.list_backups_detailed().await.expect("listing");
        assert!(listing.complete);
        assert_eq!(listing.backups.len(), 2);
        assert_eq!(
            recorded_requests(&server),
            vec!["GET /repos/owner/repo/contents/backups".to_string()]
        );
    }

    #[tokio::test]
    async fn truncated_tree_listing_reports_incompleteness() {
        let server = start_mock(vec![
            (200, contents_entries(GITEE_CONTENTS_SUSPECTED_CAP)),
            (200, r#"{"commit": {"sha": "commitsha"}}"#.to_string()),
            (
                200,
                r#"{"truncated": true, "tree": [
                    {"path": "backups/ai-toolbox-backup-20260913-120000-abc123ef.zip", "type": "blob", "sha": "blob1", "size": 5}
                ]}"#
                .to_string(),
            ),
        ]);
        let client = test_client(&server, BackupRepositoryPlatform::Gitee);

        let listing = client.list_backups_detailed().await.expect("listing");
        assert!(!listing.complete, "truncated listing must be flagged");
    }

    fn cleanup_client(server: &MockApi) -> RepositoryClient {
        RepositoryClient::for_test(
            reqwest::Client::new(),
            &BackupRepositorySettings {
                config: BackupRepositoryConfig {
                    platform: BackupRepositoryPlatform::Github,
                    owner: "owner".into(),
                    repository: "repo".into(),
                    branch: "main".into(),
                    directory: "backups".into(),
                },
                token: "test-token".into(),
            },
            server.url.clone(),
        )
    }

    #[tokio::test]
    async fn retention_cleanup_never_deletes_from_a_truncated_listing() {
        let server = start_mock(vec![
            // Contents hits the truncation threshold, trees reports truncated=true.
            (200, contents_entries(GITHUB_CONTENTS_TRUNCATION)),
            (200, r#"{"commit": {"sha": "commitsha"}}"#.to_string()),
            (
                200,
                r#"{"truncated": true, "tree": [
                    {"path": "backups/ai-toolbox-backup-20260913-120000-abc123ef.zip", "type": "blob", "sha": "blob1", "size": 5},
                    {"path": "backups/ai-toolbox-backup-20260913-120001-abc123ef.zip", "type": "blob", "sha": "blob2", "size": 5}
                ]}"#
                .to_string(),
            ),
        ]);
        let client = cleanup_client(&server);

        // Two backups over max_keep=1, but the listing is truncated: cleanup must
        // stop before any DELETE leaves the machine.
        crate::settings::backup::auto_backup::cleanup_old_repository_backups(&client, 1)
            .await
            .expect("cleanup skips without error");
        assert!(
            recorded_requests(&server)
                .iter()
                .all(|request| !request.starts_with("DELETE")),
            "no delete request may run against a truncated listing"
        );
    }

    #[tokio::test]
    async fn retention_cleanup_deletes_only_oldest_when_listing_is_complete() {
        let server = start_mock(vec![
            (200, contents_entries(2)),
            (200, r#"{"message": "deleted"}"#.to_string()),
        ]);
        let client = cleanup_client(&server);

        crate::settings::backup::auto_backup::cleanup_old_repository_backups(&client, 1)
            .await
            .expect("cleanup succeeds");
        let requests = recorded_requests(&server);
        let delete_target = requests
            .iter()
            .find(|request| request.starts_with("DELETE"))
            .expect("one delete must happen");
        // backups[1] after descending sort is the older of the two names.
        assert!(
            delete_target.ends_with("ai-toolbox-backup-20260913-120000.zip"),
            "unexpected delete target: {delete_target}"
        );
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.starts_with("DELETE"))
                .count(),
            1
        );
    }
}
