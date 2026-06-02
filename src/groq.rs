use serde::{Deserialize, Serialize};

use crate::errors::{Result, UpmError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroqConfig {
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    response_format: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildPlan {
    pub build: Vec<String>,
    pub install: Vec<String>,
    pub dependencies: Vec<String>,
    pub explanation: String,
}

pub struct Groq {
    config: GroqConfig,
    client: reqwest::Client,
}

impl Groq {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("UPM_GROQ_API_KEY")
            .or_else(|_| std::env::var("GROQ_API_KEY"))
            .ok()
            .or_else(crate::groq_key::decrypt_groq_key)?;

        Some(Self {
            config: GroqConfig {
                api_key,
                model: "llama-3.3-70b-versatile".into(),
            },
            client: reqwest::Client::new(),
        })
    }

    pub async fn generate_build_plan(
        &self,
        package_name: &str,
        repo_url: &str,
        file_list: &[String],
        platform: &str,
        preferred_tools: &[String],
    ) -> Result<BuildPlan> {
        let files = file_list.join("\n");
        let tools = if preferred_tools.is_empty() {
            "auto-detect".to_string()
        } else {
            preferred_tools.join(", ")
        };

        let prompt = format!(
            r#"You are a build system expert. Given a GitHub repository, analyze its file structure and determine how to build and install it.

Package: {package}
Repository: {repo}
Platform: {platform}
Preferred tools: {tools}

Files in repository:
{files}

Return a JSON object with:
- "build": list of shell commands to build the project (empty list if no build needed)
- "install": list of shell commands to install the built files to {{prefix}}/bin or appropriate locations
- "dependencies": list of system packages needed (e.g. ["build-essential", "cmake"])
- "explanation": brief explanation of your analysis

Rules:
- Use {{prefix}} as the installation prefix (e.g. "cp mybinary {{prefix}}/bin/")
- If it's a Makefile project: "make -j$(nproc)" and "make install DESTDIR={{prefix}}"
- If it's Cargo: "cargo build --release" and "cp target/release/{{binary}} {{prefix}}/bin/"
- If it's a script: just "cp {{script}} {{prefix}}/bin/"
- If it's a Go project: "go build -o {{binary}}" and "cp {{binary}} {{prefix}}/bin/"
- If it's npm: "npm install" and "cp -r . {{prefix}}/lib/node_modules/{{package}}"
- If it's Python: "pip install ." or "python setup.py install"
- If it's CMake: "cmake . -DCMAKE_INSTALL_PREFIX={{prefix}}" then "make -j$(nproc)" then "make install"
- For single C/C++ files: "cc -o {{binary}} {{file}}.c" and "cp {{binary}} {{prefix}}/bin/"
- For configure-based: "./configure --prefix={{prefix}}" then "make -j$(nproc)" then "make install"
- For static site / docs only: empty build, install is "cp -r . {{prefix}}/share/{{package}}/"
"#,
            package = package_name,
            repo = repo_url,
            platform = platform,
            tools = tools,
            files = files,
        );

        let body = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: "You are a build system expert. Respond only with valid JSON.".into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: prompt,
                },
            ],
            temperature: 0.1,
            max_tokens: 2000,
            response_format: serde_json::json!({"type": "json_object"}),
        };

        let resp = self
            .client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| UpmError::General(format!("Groq API request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(UpmError::General(format!(
                "Groq API error ({}): {}",
                status, text
            )));
        }

        let chat: ChatResponse = resp
            .json()
            .await
            .map_err(|e| UpmError::General(format!("Failed to parse Groq response: {e}")))?;

        let content = chat
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let clean: String = content
            .chars()
            .filter(|c| c.is_ascii_graphic() || c.is_ascii_whitespace())
            .collect();

        let plan: BuildPlan = serde_json::from_str(&clean)
            .map_err(|e| UpmError::General(format!("Failed to parse AI build plan: {e}\nRaw: {}", &content[..content.len().min(200)])))?;

        Ok(plan)
    }

    pub fn list_directory_tree(root: &std::path::Path, max_depth: usize) -> Result<Vec<String>> {
        let mut files = Vec::new();

        if !root.exists() {
            return Ok(files);
        }

        Self::walk_dir(root, root, 0, max_depth, &mut files)?;

        Ok(files)
    }

    fn walk_dir(
        base: &std::path::Path,
        dir: &std::path::Path,
        depth: usize,
        max_depth: usize,
        files: &mut Vec<String>,
    ) -> Result<()> {
        if depth > max_depth {
            return Ok(());
        }

        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| UpmError::General(format!("Cannot read dir {}: {e}", dir.display())))?
            .filter_map(|e| e.ok())
            .collect();

        entries.sort_by_key(|e| e.file_name());

        for entry in &entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            if relative.starts_with('.') || relative == "target" || relative == "node_modules"
                || relative.starts_with("target/") || relative.starts_with("node_modules/")
            {
                continue;
            }

            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                files.push(format!("{}/", relative));
                Self::walk_dir(base, &path, depth + 1, max_depth, files)?;
            } else {
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                files.push(format!("{} ({} bytes)", relative, size));
            }
        }

        Ok(())
    }
}
