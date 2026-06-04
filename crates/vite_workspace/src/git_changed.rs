use std::process::Command;

use rustc_hash::FxHashSet;
use vite_path::{AbsolutePath, AbsolutePathBuf};
use vite_str::{Str, format};

use crate::package_graph::PackageQueryResolveError;

pub(crate) fn changed_files_since_ref(
    workspace_root: &AbsolutePath,
    base_ref: &str,
) -> Result<Vec<AbsolutePathBuf>, PackageQueryResolveError> {
    let repo_root = git_show_toplevel(workspace_root).map_err(|message| {
        PackageQueryResolveError::GitDiffFailed { base_ref: base_ref.into(), message }
    })?;

    let diff = git_diff_name_only(workspace_root, base_ref).map_err(|message| {
        PackageQueryResolveError::GitDiffFailed { base_ref: base_ref.into(), message }
    })?;

    if diff.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    // Dedupe while preserving order (stable-ish for tests).
    let mut seen = FxHashSet::<AbsolutePathBuf>::default();

    for raw_line in diff.lines() {
        let raw_line = raw_line.trim();
        if raw_line.is_empty() {
            continue;
        }

        // The prefix and suffix '"' can be appended to some paths.
        let line = raw_line.trim_start_matches('"').trim_end_matches('"');
        if line.is_empty() {
            continue;
        }

        // `AbsolutePathBuf::push` (used by join) handles both relative paths and
        // absolute paths (absolute replaces the base), so we don't need to inspect
        // std path types here (they're clippy-disallowed outside `vite_path`).
        let abs = repo_root.join(line);

        if seen.insert(abs.clone()) {
            out.push(abs);
        }
    }

    Ok(out)
}

fn git_show_toplevel(cwd: &AbsolutePath) -> Result<AbsolutePathBuf, Str> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd.as_path())
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .map_err(|e| format!("failed to spawn git: {e}"))?;

    if !output.status.success() {
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(format!("git rev-parse failed: {}", stderr.trim()));
    }

    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|e| format!("git rev-parse output was not utf-8: {e}"))?;
    let toplevel = stdout.trim();
    let abs = AbsolutePath::new(toplevel)
        .ok_or_else(|| format!("git returned non-absolute repo root: {toplevel}"))?;
    Ok(abs.to_absolute_path_buf())
}

fn git_diff_name_only(cwd: &AbsolutePath, base_ref: &str) -> Result<Str, Str> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd.as_path())
        .arg("diff")
        .arg("--name-only")
        .arg(base_ref)
        .arg("--")
        .arg(".")
        .output()
        .map_err(|e| format!("failed to spawn git: {e}"))?;

    if !output.status.success() {
        let stderr = std::str::from_utf8(&output.stderr).unwrap_or("<non-utf8 stderr>");
        return Err(format!("git diff failed: {}", stderr.trim()));
    }

    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|e| format!("git diff output was not utf-8: {e}"))?;
    Ok(Str::from(stdout))
}
