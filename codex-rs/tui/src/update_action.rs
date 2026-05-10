#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::StandalonePlatform;

const OPENAI_CODEX_PACKAGE: &str = "@openai/codex";
const DISTRIBUTION_PACKAGE_ENV: &str = "CODEX_DISTRIBUTION_PACKAGE";
const DISTRIBUTION_REPOSITORY_ENV: &str = "CODEX_DISTRIBUTION_REPOSITORY";
const DISTRIBUTION_VERSION_ENV: &str = "CODEX_DISTRIBUTION_VERSION";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DistributionInfo {
    pub(crate) package: String,
    pub(crate) version: Option<String>,
    pub(crate) github_repo: Option<String>,
}

/// Update action the CLI should perform after the TUI exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update via `npm install -g @openai/codex@latest`.
    NpmGlobalLatest,
    /// Update via `bun install -g @openai/codex@latest`.
    BunGlobalLatest,
    /// Update via `brew upgrade codex`.
    BrewUpgrade,
    /// Update via `curl -fsSL https://chatgpt.com/codex/install.sh | sh`.
    StandaloneUnix,
    /// Update via `irm https://chatgpt.com/codex/install.ps1|iex`.
    StandaloneWindows,
}

impl UpdateAction {
    #[cfg(any(not(debug_assertions), test))]
    pub(crate) fn from_install_context(context: &InstallContext) -> Option<Self> {
        match context {
            InstallContext::Npm => Some(UpdateAction::NpmGlobalLatest),
            InstallContext::Bun => Some(UpdateAction::BunGlobalLatest),
            InstallContext::Brew => Some(UpdateAction::BrewUpgrade),
            InstallContext::Standalone { platform, .. } => Some(match platform {
                StandalonePlatform::Unix => UpdateAction::StandaloneUnix,
                StandalonePlatform::Windows => UpdateAction::StandaloneWindows,
            }),
            InstallContext::Other => None,
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::NpmGlobalLatest => ("npm", &["install", "-g", "@openai/codex"]),
            UpdateAction::BunGlobalLatest => ("bun", &["install", "-g", "@openai/codex"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "--cask", "codex"]),
            UpdateAction::StandaloneUnix => (
                "sh",
                &["-c", "curl -fsSL https://chatgpt.com/codex/install.sh | sh"],
            ),
            UpdateAction::StandaloneWindows => (
                "powershell",
                &["-c", "irm https://chatgpt.com/codex/install.ps1|iex"],
            ),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

#[cfg(not(debug_assertions))]
pub(crate) fn is_third_party_distribution() -> bool {
    std::env::var(DISTRIBUTION_PACKAGE_ENV).is_ok_and(|package| package != OPENAI_CODEX_PACKAGE)
}

pub(crate) fn third_party_distribution() -> Option<DistributionInfo> {
    let package = std::env::var(DISTRIBUTION_PACKAGE_ENV).ok()?;
    if package == OPENAI_CODEX_PACKAGE {
        return None;
    }

    Some(DistributionInfo {
        package,
        version: std::env::var(DISTRIBUTION_VERSION_ENV).ok(),
        github_repo: std::env::var(DISTRIBUTION_REPOSITORY_ENV)
            .ok()
            .and_then(|repo| github_repo_slug(&repo)),
    })
}

pub(crate) fn current_version(default_version: &str) -> String {
    third_party_distribution()
        .and_then(|info| info.version)
        .unwrap_or_else(|| default_version.to_string())
}

pub(crate) fn release_notes_url() -> String {
    third_party_distribution()
        .and_then(|info| info.github_repo)
        .map(|repo| format!("https://github.com/{repo}/releases/latest"))
        .unwrap_or_else(|| "https://github.com/openai/codex/releases/latest".to_string())
}

pub(crate) fn installation_url() -> String {
    third_party_distribution()
        .and_then(|info| info.github_repo)
        .map(|repo| format!("https://github.com/{repo}"))
        .unwrap_or_else(|| "https://github.com/openai/codex".to_string())
}

fn github_repo_slug(repository: &str) -> Option<String> {
    let trimmed = repository
        .trim()
        .trim_start_matches("git+")
        .trim_end_matches(".git");
    let after_host = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("git@github.com:"))?;
    let mut parts = after_host.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() {
        None
    } else {
        Some(format!("{owner}/{repo}"))
    }
}

#[cfg(not(debug_assertions))]
pub fn get_update_action() -> Option<UpdateAction> {
    if is_third_party_distribution() {
        return None;
    }

    UpdateAction::from_install_context(InstallContext::current())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn maps_install_context_to_update_action() {
        let native_release_dir = PathBuf::from("/tmp/native-release");

        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Other),
            None
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Npm),
            Some(UpdateAction::NpmGlobalLatest)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Bun),
            Some(UpdateAction::BunGlobalLatest)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Brew),
            Some(UpdateAction::BrewUpgrade)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Standalone {
                platform: StandalonePlatform::Unix,
                release_dir: native_release_dir.clone(),
                resources_dir: Some(native_release_dir.join("codex-resources")),
            }),
            Some(UpdateAction::StandaloneUnix)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Standalone {
                platform: StandalonePlatform::Windows,
                release_dir: native_release_dir.clone(),
                resources_dir: Some(native_release_dir.join("codex-resources")),
            }),
            Some(UpdateAction::StandaloneWindows)
        );
    }

    #[test]
    fn standalone_update_commands_rerun_latest_installer() {
        assert_eq!(
            UpdateAction::StandaloneUnix.command_args(),
            (
                "sh",
                &["-c", "curl -fsSL https://chatgpt.com/codex/install.sh | sh"][..],
            )
        );
        assert_eq!(
            UpdateAction::StandaloneWindows.command_args(),
            (
                "powershell",
                &["-c", "irm https://chatgpt.com/codex/install.ps1|iex"][..],
            )
        );
    }
}
