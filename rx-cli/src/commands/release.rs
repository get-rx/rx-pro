//! Release command - version bumping, tagging, and publishing workflow

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};
use clap::Args;

use rx_core::pep::PyProject;
use rx_core::versioning::bump_version;

#[derive(Args)]
pub struct ReleaseCommand {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".", global = true)]
    pub project: PathBuf,

    /// Version bump type: major, minor, patch, or pre
    #[arg(long)]
    pub bump: Option<String>,

    /// Set a specific version instead of bumping
    #[arg(long, conflicts_with = "bump")]
    pub version: Option<String>,

    /// Skip changelog generation
    #[arg(long)]
    pub no_changelog: bool,

    /// Skip git tag creation
    #[arg(long)]
    pub no_tag: bool,

    /// Skip git commit
    #[arg(long)]
    pub no_commit: bool,

    /// Push to remote after release
    #[arg(long)]
    pub push: bool,

    /// Publish to PyPI after release
    #[arg(long)]
    pub publish: bool,

    /// Tag prefix (default: "v")
    #[arg(long, default_value = "v")]
    pub tag_prefix: String,

    /// Don't actually make changes, just show what would happen
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation prompts
    #[arg(long, short = 'y')]
    pub yes: bool,
}

impl ReleaseCommand {
    pub async fn run(self) -> Result<()> {
        let project_dir = if self.project.as_os_str() == "." {
            std::env::current_dir()?
        } else {
            self.project.canonicalize()?
        };

        // Load pyproject.toml
        let pyproject = PyProject::load(&project_dir).with_context(|| {
            format!(
                "No pyproject.toml found in {:?}. Run 'rx init' first.",
                project_dir
            )
        })?;

        let project_name = pyproject.name().unwrap_or("unknown").to_string();
        let current_version = pyproject
            .version()
            .context("No version in pyproject.toml")?
            .to_string();

        // Determine new version
        let new_version = if let Some(ref v) = self.version {
            v.clone()
        } else if let Some(ref bump_type) = self.bump {
            bump_version(&current_version, bump_type)
                .with_context(|| format!("Failed to bump {} version", bump_type))?
        } else {
            // Interactive mode - ask user
            prompt_bump_type(&current_version)?
        };

        // Show release plan
        println!();
        println!("Release Plan for {}", project_name);
        println!("─────────────────────────────────");
        println!("  Version: {} → {}", current_version, new_version);
        println!("  Tag:     {}{}", self.tag_prefix, new_version);
        if !self.no_changelog {
            println!("  Changelog: Will be updated");
        }
        if !self.no_commit {
            println!("  Commit: Will create release commit");
        }
        if !self.no_tag {
            println!("  Git tag: Will be created");
        }
        if self.push {
            println!("  Push: Will push to remote");
        }
        if self.publish {
            println!("  Publish: Will publish to PyPI");
        }
        println!();

        if self.dry_run {
            println!("Dry run - no changes made.");
            return Ok(());
        }

        // Confirm
        if !self.yes && !confirm("Proceed with release?")? {
            println!("Aborted.");
            return Ok(());
        }

        // Check for uncommitted changes
        if !self.no_commit && has_uncommitted_changes(&project_dir)? {
            bail!("Working directory has uncommitted changes. Commit or stash them first.");
        }

        // Step 1: Update version in pyproject.toml
        println!("Updating version in pyproject.toml...");
        let mut updated_pyproject = pyproject.clone();
        if let Some(ref mut project) = updated_pyproject.project {
            project.version = Some(new_version.clone());
        }
        updated_pyproject.save(&project_dir)?;
        println!("  ✓ Updated pyproject.toml");

        // Step 2: Generate changelog
        let changelog_entry = if !self.no_changelog {
            println!("Generating changelog...");
            let entry = generate_changelog(&project_dir, &current_version, &new_version)?;
            if let Some(ref entry) = entry {
                update_changelog(&project_dir, &new_version, entry)?;
                println!("  ✓ Updated CHANGELOG.md");
            } else {
                println!("  ⚠ No conventional commits found, skipping changelog");
            }
            entry
        } else {
            None
        };

        // Step 3: Create commit
        if !self.no_commit {
            println!("Creating release commit...");
            create_release_commit(&project_dir, &new_version, changelog_entry.is_some())?;
            println!("  ✓ Created commit");
        }

        // Step 4: Create tag
        if !self.no_tag {
            let tag_name = format!("{}{}", self.tag_prefix, new_version);
            println!("Creating git tag {}...", tag_name);
            create_git_tag(&project_dir, &tag_name, &format!("Release {}", new_version))?;
            println!("  ✓ Created tag {}", tag_name);
        }

        // Step 5: Push
        if self.push {
            println!("Pushing to remote...");
            push_to_remote(&project_dir, !self.no_tag)?;
            println!("  ✓ Pushed to remote");
        }

        // Step 6: Publish
        if self.publish {
            println!("Publishing to PyPI...");
            println!("  ⚠ Publishing not yet implemented (rx build && rx publish)");
            // TODO: Call rx build and rx publish
        }

        println!();
        println!("✓ Released {} {}", project_name, new_version);

        if !self.push {
            println!();
            println!("Next steps:");
            println!("  git push --follow-tags    # Push commit and tag");
            if !self.publish {
                println!("  rx build && rx publish    # Publish to PyPI");
            }
        }

        Ok(())
    }
}

fn prompt_bump_type(current: &str) -> Result<String> {
    println!("Current version: {}", current);
    println!();
    println!("Select version bump:");
    println!("  1. patch  ({} → {})", current, bump_version(current, "patch").unwrap_or_default());
    println!("  2. minor  ({} → {})", current, bump_version(current, "minor").unwrap_or_default());
    println!("  3. major  ({} → {})", current, bump_version(current, "major").unwrap_or_default());
    println!("  4. pre    ({} → {})", current, bump_version(current, "pre").unwrap_or_default());
    println!("  5. custom (enter version)");
    println!();
    print!("Choice [1-5]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim() {
        "1" | "patch" => Ok(bump_version(current, "patch")?),
        "2" | "minor" => Ok(bump_version(current, "minor")?),
        "3" | "major" => Ok(bump_version(current, "major")?),
        "4" | "pre" => Ok(bump_version(current, "pre")?),
        "5" | "custom" => {
            print!("Enter version: ");
            io::stdout().flush()?;
            let mut version = String::new();
            io::stdin().read_line(&mut version)?;
            Ok(version.trim().to_string())
        }
        _ => bail!("Invalid choice"),
    }
}

fn confirm(message: &str) -> Result<bool> {
    print!("{} [y/N]: ", message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(matches!(input.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn has_uncommitted_changes(project_dir: &std::path::Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_dir)
        .output()
        .context("Failed to run git status")?;

    Ok(!output.stdout.is_empty())
}

/// Commit types for conventional commits
struct ConventionalCommit {
    commit_type: String,
    scope: Option<String>,
    description: String,
    breaking: bool,
}

fn parse_conventional_commit(message: &str) -> Option<ConventionalCommit> {
    // Format: type(scope)!: description
    // or:     type!: description
    // or:     type(scope): description
    // or:     type: description
    let re = regex::Regex::new(
        r"^(?P<type>\w+)(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?: (?P<desc>.+)"
    ).ok()?;

    let caps = re.captures(message.lines().next()?)?;

    Some(ConventionalCommit {
        commit_type: caps.name("type")?.as_str().to_string(),
        scope: caps.name("scope").map(|m: regex::Match| m.as_str().to_string()),
        description: caps.name("desc")?.as_str().to_string(),
        breaking: caps.name("breaking").is_some() || message.contains("BREAKING CHANGE:"),
    })
}

fn generate_changelog(
    project_dir: &std::path::Path,
    from_version: &str,
    to_version: &str,
) -> Result<Option<String>> {
    // Get commits since last tag
    let from_tag = format!("v{}", from_version);

    // Try to get commits since last tag, fall back to all commits
    let output = Command::new("git")
        .args(["log", "--pretty=format:%s", &format!("{}..HEAD", from_tag)])
        .current_dir(project_dir)
        .output();

    let commits: Vec<String> = match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect()
        }
        _ => {
            // No previous tag, get recent commits
            let out = Command::new("git")
                .args(["log", "--pretty=format:%s", "-20"])
                .current_dir(project_dir)
                .output()
                .context("Failed to get git log")?;

            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect()
        }
    };

    if commits.is_empty() {
        return Ok(None);
    }

    // Parse and categorize commits
    let mut features: Vec<String> = Vec::new();
    let mut fixes: Vec<String> = Vec::new();
    let mut breaking: Vec<String> = Vec::new();
    let mut other: Vec<String> = Vec::new();

    for commit in &commits {
        if let Some(conv) = parse_conventional_commit(commit) {
            let entry = if let Some(scope) = conv.scope {
                format!("**{}**: {}", scope, conv.description)
            } else {
                conv.description.clone()
            };

            if conv.breaking {
                breaking.push(entry.clone());
            }

            match conv.commit_type.as_str() {
                "feat" | "feature" => features.push(entry),
                "fix" => fixes.push(entry),
                "docs" | "style" | "refactor" | "perf" | "test" | "chore" | "ci" | "build" => {
                    // Skip these in changelog
                }
                _ => other.push(entry),
            }
        }
    }

    if features.is_empty() && fixes.is_empty() && breaking.is_empty() {
        return Ok(None);
    }

    // Generate changelog entry
    let date = chrono_date();
    let mut entry = format!("## [{}] - {}\n\n", to_version, date);

    if !breaking.is_empty() {
        entry.push_str("### ⚠ Breaking Changes\n\n");
        for item in &breaking {
            entry.push_str(&format!("- {}\n", item));
        }
        entry.push('\n');
    }

    if !features.is_empty() {
        entry.push_str("### Features\n\n");
        for item in &features {
            entry.push_str(&format!("- {}\n", item));
        }
        entry.push('\n');
    }

    if !fixes.is_empty() {
        entry.push_str("### Bug Fixes\n\n");
        for item in &fixes {
            entry.push_str(&format!("- {}\n", item));
        }
        entry.push('\n');
    }

    Ok(Some(entry))
}

fn chrono_date() -> String {
    // Simple date without chrono dependency
    let output = Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok();

    output
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "YYYY-MM-DD".to_string())
}

fn update_changelog(
    project_dir: &std::path::Path,
    _version: &str,
    entry: &str,
) -> Result<()> {
    let changelog_path = project_dir.join("CHANGELOG.md");

    let existing = if changelog_path.exists() {
        std::fs::read_to_string(&changelog_path)?
    } else {
        "# Changelog\n\nAll notable changes to this project will be documented in this file.\n\n".to_string()
    };

    // Insert new entry after the header
    let new_content = if let Some(idx) = existing.find("\n## ") {
        // Insert before first version section
        let (header, rest) = existing.split_at(idx);
        format!("{}\n{}{}", header, entry, rest)
    } else if existing.contains("# Changelog") {
        // No versions yet, append after header
        format!("{}\n{}", existing.trim_end(), entry)
    } else {
        // No header, create new file
        format!("# Changelog\n\nAll notable changes to this project will be documented in this file.\n\n{}", entry)
    };

    std::fs::write(&changelog_path, new_content)?;
    Ok(())
}

fn create_release_commit(
    project_dir: &std::path::Path,
    version: &str,
    include_changelog: bool,
) -> Result<()> {
    // Stage files
    let mut files = vec!["pyproject.toml"];
    if include_changelog {
        files.push("CHANGELOG.md");
    }

    Command::new("git")
        .args(["add"])
        .args(&files)
        .current_dir(project_dir)
        .output()
        .context("Failed to stage files")?;

    // Create commit
    let message = format!("chore(release): {}", version);
    let output = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(project_dir)
        .output()
        .context("Failed to create commit")?;

    if !output.status.success() {
        bail!(
            "Failed to create commit: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn create_git_tag(project_dir: &std::path::Path, tag: &str, message: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["tag", "-a", tag, "-m", message])
        .current_dir(project_dir)
        .output()
        .context("Failed to create tag")?;

    if !output.status.success() {
        bail!(
            "Failed to create tag: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn push_to_remote(project_dir: &std::path::Path, push_tags: bool) -> Result<()> {
    // Push commits
    let output = Command::new("git")
        .args(["push"])
        .current_dir(project_dir)
        .output()
        .context("Failed to push")?;

    if !output.status.success() {
        bail!(
            "Failed to push: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Push tags
    if push_tags {
        let output = Command::new("git")
            .args(["push", "--tags"])
            .current_dir(project_dir)
            .output()
            .context("Failed to push tags")?;

        if !output.status.success() {
            bail!(
                "Failed to push tags: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    Ok(())
}
