use crate::Res;
use std::{
    error::Error,
    fs,
    io::ErrorKind,
    path::Path,
    process::Command,
    str::{self, FromStr},
};

use self::{diff::Diff, rebase_status::RebaseStatus};

pub(crate) mod diff;
mod parse;
pub(crate) mod rebase_status;
pub(crate) mod status;

// TODO Check for.git/index.lock and block if it exists
// TODO Use only plumbing commands

pub(crate) fn rebase_status(dir: &Path) -> Res<Option<RebaseStatus>> {
    let mut rebase_onto_file = dir.to_path_buf();
    rebase_onto_file.push(".git/rebase-merge/onto");

    let mut rebase_head_name_file = dir.to_path_buf();
    rebase_head_name_file.push(".git/rebase-merge/head-name");

    match fs::read_to_string(&rebase_onto_file) {
        Ok(content) => {
            let onto_hash = content.trim().to_string();
            Ok(Some(RebaseStatus {
                onto: branch_name(dir, &onto_hash)?.unwrap_or_else(|| onto_hash[..7].to_string()),
                head_name: fs::read_to_string(rebase_head_name_file)?
                    .trim()
                    .strip_prefix("refs/heads/")
                    .unwrap()
                    .to_string(),
                // TODO include log of 'done' items
            }))
        }
        Err(err) => {
            if err.kind() == ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(Box::new(err))
            }
        }
    }
}

fn branch_name(dir: &Path, hash: &str) -> Res<Option<String>> {
    let out = Command::new("git")
        .args(["for-each-ref", "--format", "%(objectname) %(refname:short)"])
        .current_dir(dir)
        .output()?
        .stdout;

    Ok(str::from_utf8(&out)?
        .lines()
        .find(|line| line.starts_with(hash))
        .map(|line| line.split(' ').nth(1).unwrap().to_string()))
}

pub(crate) fn diff(dir: &Path, args: &[&str]) -> Res<Diff> {
    run_git(dir, &["diff"], args)
}

pub(crate) fn diff_unstaged(dir: &Path) -> Res<Diff> {
    run_git(dir, &["diff"], &[])
}

pub(crate) fn diff_staged(dir: &Path) -> Res<Diff> {
    run_git(dir, &["diff", "--staged"], &[])
}

pub(crate) fn status(dir: &Path) -> Res<status::Status> {
    run_git(dir, &["status", "--porcelain", "--branch"], &[])
}

pub(crate) fn show(dir: &Path, args: &[&str]) -> Res<Diff> {
    run_git(dir, &["show"], args)
}

pub(crate) fn show_summary(dir: &Path, args: &[&str]) -> Res<String> {
    run_git_no_parse(dir, &["show", "--summary", "--decorate", "--color"], args)
}

// TODO Make this return a more useful type. Vec<Log>?
pub(crate) fn log_recent(dir: &Path) -> Res<String> {
    run_git_no_parse(
        dir,
        &["log", "-n", "5", "--oneline", "--decorate", "--color"],
        &[],
    )
}
// TODO Make this return a more useful type. Vec<Log>?
pub(crate) fn log(dir: &Path, args: &[&str]) -> Res<String> {
    run_git_no_parse(dir, &["log", "--oneline", "--decorate", "--color"], args)
}

// TODO Clean this up
pub(crate) fn show_refs(dir: &Path) -> Res<Vec<(String, String, String)>> {
    let out = Command::new("git")
        .args([
            "for-each-ref",
            "--sort",
            "-creatordate",
            "--format",
            "%(refname:short) %(upstream:short) %(subject)",
            "refs/heads",
        ])
        .current_dir(dir)
        .output()?
        .stdout;

    Ok(str::from_utf8(&out)?
        .lines()
        .map(|line| {
            let mut columns = line.splitn(3, ' ');
            let local = columns.next().unwrap().to_string();
            let remote = columns.next().unwrap().to_string();
            let subject = columns.next().unwrap().to_string();

            (local, remote, subject)
        })
        .collect())
}

pub(crate) fn stage_file_cmd(file: &str) -> Command {
    git(&["add", file])
}
pub(crate) fn stage_patch_cmd() -> Command {
    git(&["apply", "--cached"])
}
pub(crate) fn unstage_file_cmd(file: &str) -> Command {
    git(&["restore", "--staged", file])
}
pub(crate) fn unstage_patch_cmd() -> Command {
    git(&["apply", "--cached", "--reverse"])
}
pub(crate) fn discard_unstaged_patch_cmd() -> Command {
    git(&["apply", "--reverse"])
}
pub(crate) fn commit_cmd() -> Command {
    git(&["commit"])
}
pub(crate) fn commit_amend_cmd() -> Command {
    git(&["commit", "--amend"])
}
pub(crate) fn commit_fixup_cmd(reference: &str) -> Command {
    git(&["commit", "--fixup", reference])
}
pub(crate) fn push_cmd() -> Command {
    git(&["push"])
}
pub(crate) fn pull_cmd() -> Command {
    git(&["pull"])
}
pub(crate) fn fetch_all_cmd() -> Command {
    git(&["fetch", "--all"])
}
pub(crate) fn rebase_interactive_cmd(reference: &str) -> Command {
    // TODO autostash flag should be visible as a flag (though set as default)
    git(&["rebase", "-i", "--autostash", reference])
}
pub(crate) fn rebase_autosquash_cmd(reference: &str) -> Command {
    // TODO autostash flag should be visible as a flag (though set as default)
    git(&[
        "rebase",
        "-i",
        "--autosquash",
        "--keep-empty",
        "--autostash",
        reference,
    ])
}
pub(crate) fn rebase_continue_cmd() -> Command {
    git(&["rebase", "--continue"])
}
pub(crate) fn rebase_abort_cmd() -> Command {
    git(&["rebase", "--abort"])
}

pub(crate) fn checkout_file_cmd(file: &str) -> Command {
    git(&["checkout", "--", file])
}

pub(crate) fn checkout_ref_cmd(reference: &str) -> Command {
    git(&["checkout", reference])
}

fn run_git<T: FromStr<Err = Box<dyn Error>>>(
    dir: &Path,
    args: &[&str],
    meta_args: &[&str],
) -> Res<T> {
    let out = Command::new("git")
        .args(&[args, meta_args].concat())
        .current_dir(dir)
        .output()?
        .stdout;

    str::from_utf8(&out)?.parse()
}

fn run_git_no_parse(dir: &Path, args: &[&str], meta_args: &[&str]) -> Res<String> {
    let out = Command::new("git")
        .args(&[args, meta_args].concat())
        .current_dir(dir)
        .output()?
        .stdout;

    Ok(String::from_utf8(out)?)
}

fn git(args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(args);
    cmd
}
