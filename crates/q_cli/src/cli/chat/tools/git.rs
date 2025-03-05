use std::collections::HashMap;
use std::io::Write;
use std::process::Stdio;

use bstr::ByteSlice;
use crossterm::{
    queue,
    style,
};
use eyre::{
    Context as _,
    Result,
};
use fig_os_shim::Context;
use serde::Deserialize;

use super::{
    InvokeOutput,
    OutputKind,
};

const READONLY_COMMANDS: [&str; 12] = [
    "status",
    "log",
    "show",
    "diff",
    "grep",
    "ls-files",
    "ls-remote",
    "rev-parse",
    "blame",
    "describe",
    "cat-file",
    "check-ignore",
];

const READONLY_SUBCOMMANDS: [(&str, &str); 4] = [
    ("config", "get"),
    ("config", "list"),
    ("remote", "show"),
    ("remote", "get-url"),
];

#[derive(Debug, Clone, Deserialize)]
pub struct Git {
    pub command: String,
    pub subcommand: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub parameters: Option<HashMap<String, serde_json::Value>>,
    pub label: Option<String>,
}

impl Git {
    pub fn requires_consent(&self) -> bool {
        if READONLY_COMMANDS.contains(&self.command.as_str()) {
            return false;
        }

        match (self.command.as_str(), self.subcommand.as_ref()) {
            ("branch" | "tag" | "remote", None)
                if self.parameters.is_none() || self.parameters.as_ref().is_some_and(|p| p.is_empty()) =>
            {
                false
            },
            (cmd, Some(sub_command)) => !READONLY_SUBCOMMANDS.contains(&(cmd, sub_command)),
            _ => true,
        }
    }

    pub async fn invoke(&self, _ctx: &Context, _updates: impl Write) -> Result<InvokeOutput> {
        let mut command = tokio::process::Command::new("git");
        command.arg(&self.command);
        if let Some(subcommand) = self.subcommand.as_ref() {
            command.arg(subcommand);
        }
        if let Some(repo) = self.repo.as_ref() {
            command.arg(repo);
        }
        if let Some(branch) = self.branch.as_ref() {
            command.arg(branch);
        }
        if let Some(parameters) = self.parameters.as_ref() {
            for (name, val) in parameters {
                let param_name = format!("--{}", name.trim_start_matches("--"));
                let param_val = val.as_str().map(|s| s.to_string()).unwrap_or(val.to_string());
                command.arg(param_name).arg(param_val);
            }
        }
        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .wrap_err_with(|| format!("Unable to spawn command '{:?}'", self))?
            .wait_with_output()
            .await
            .wrap_err_with(|| format!("Unable to spawn command '{:?}'", self))?;
        let status = output.stdout.to_str_lossy();
        let stdout = output.stdout.to_str_lossy();
        let stderr = output.stderr.to_str_lossy();

        Ok(InvokeOutput {
            output: OutputKind::Json(serde_json::json!({
                "exit_status": status,
                "stdout": stdout,
                "stderr": stderr.clone()
            })),
        })
    }

    pub fn queue_description(&self, updates: &mut impl Write) -> Result<()> {
        if let Some(label) = self.label.as_ref() {
            queue!(updates, style::Print(label))?;
        }
        queue!(
            updates,
            style::Print("\n"),
            style::Print(format!("Command: git {}", self.command))
        )?;
        if let Some(subcommand) = self.subcommand.as_ref() {
            queue!(updates, style::Print(format!(" {}", subcommand)))?;
        }
        if let Some(repo) = self.repo.as_ref() {
            queue!(updates, style::Print(format!(" {}", repo)))?;
        }
        if let Some(branch) = self.branch.as_ref() {
            queue!(updates, style::Print(format!(" {}", branch)))?;
        }
        if let Some(parameters) = self.parameters.as_ref() {
            for (name, val) in parameters {
                let param_name = format!("--{}", name.trim_start_matches("--"));
                let param_val = val.as_str().map(|s| s.to_string()).unwrap_or(val.to_string());
                queue!(updates, style::Print(format!(" {} {}", param_name, param_val)))?;
            }
        }
        Ok(())
    }

    pub async fn validate(&mut self, _ctx: &Context) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! git {
        ($value:tt) => {
            serde_json::from_value::<Git>(serde_json::json!($value)).unwrap()
        };
    }

    #[test]
    fn test_requires_consent() {
        let readonly_commands = [
            git!({"command": "log"}),
            git!({"command": "status"}),
            git!({"command": "diff"}),
            git!({"command": "show"}),
            git!({"command": "ls-files"}),
            git!({"command": "branch"}),
            git!({"command": "tag"}),
            git!({"command": "remote"}),
            git!({"command": "blame", "parameters": {"file": "src/main.rs"}}),
            git!({"command": "rev-parse", "parameters": {"show-toplevel": true}}),
            git!({"command": "ls-remote", "repo": "origin"}),
            git!({"command": "config", "subcommand": "get", "parameters": {"name": "user.email"}}),
            git!({"command": "config", "subcommand": "list"}),
            git!({"command": "describe", "parameters": {"tags": true}}),
        ];

        for cmd in readonly_commands {
            assert!(!cmd.requires_consent(), "Command should not require consent: {:?}", cmd);
        }

        let write_commands = [
            git!({"command": "commit", "parameters": {"message": "Initial commit"}}),
            git!({"command": "push", "repo": "origin", "branch": "main"}),
            git!({"command": "pull", "repo": "origin", "branch": "main"}),
            git!({"command": "merge", "branch": "feature"}),
            git!({"command": "branch", "subcommand": "create", "branch": "new-feature"}),
            git!({"command": "branch", "parameters": {"-D": true}, "branch": "old-feature"}),
            git!({"command": "branch", "parameters": {"--delete": true}, "branch": "old-feature"}),
            git!({"command": "checkout", "branch": "develop"}),
            git!({"command": "switch", "branch": "develop"}),
            git!({"command": "reset", "parameters": {"hard": true}}),
            git!({"command": "clean", "parameters": {"-fd": true}}),
            git!({"command": "clone", "repo": "https://github.com/user/repo.git"}),
            git!({"command": "remote", "subcommand": "add", "repo": "https://github.com/user/repo.git", "parameters": {"name": "upstream"}}),
            git!({"command": "config", "subcommand": "set", "parameters": {"name": "user.email", "value": "email@example.com"}}),
        ];

        for cmd in write_commands {
            assert!(cmd.requires_consent(), "Command should require consent: {:?}", cmd);
        }
    }
}
