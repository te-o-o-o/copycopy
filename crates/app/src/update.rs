//! Updates itself from the GitHub releases. macOS only for now: the installer
//! on Windows and the AppImage or package on Linux each need their own way.
//!
//! A minute after launch, then once a day, the resident asks GitHub which
//! release is the latest. When it is newer, the disk image is downloaded, the
//! app inside is checked against our own code signature, and it replaces the
//! running bundle. The restart waits until the window is closed (`State`).
//!
//! `curl`, `hdiutil`, `ditto` and `codesign` ship with macOS: no HTTP crate.
//! A file curl downloads carries no quarantine flag, so Gatekeeper does not
//! stop the new version.
//!
//! The download is trusted only if it satisfies the running app's designated
//! requirement: same identifier, same signing certificate. That requirement is
//! also what macOS ties the Accessibility grant to, so auto-paste survives the
//! update. An ad-hoc signed app (a local build) has no such requirement, only
//! its own hash, so it never updates itself: install a release by hand once.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Mirrors `Config::auto_update`, read by the update thread before each check.
pub static ENABLED: AtomicBool = AtomicBool::new(true);

const REPO: &str = "https://github.com/te-o-o-o/copycopy";
const ASSET: &str = "copycopy-macos-universal.dmg";
const FIRST_CHECK: Duration = Duration::from_secs(60);
const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether this system can update itself, for the settings.
pub fn available() -> bool {
    cfg!(target_os = "macos")
}

/// The `.app` this process runs from, or `None` for a bare binary
/// (`cargo run`, `--demo` from `target/`).
pub fn own_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundle = exe.ancestors().nth(3)?; // copycopy.app/Contents/MacOS/copycopy
    (bundle.extension()? == "app").then(|| bundle.to_path_buf())
}

/// Starts the daily check. Sends `Wake::Updated` once a new version is in
/// place, then stops: the next one is for the restarted process.
pub fn spawn() {
    if !available() || own_bundle().is_none() {
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(FIRST_CHECK);
        loop {
            if ENABLED.load(Ordering::Relaxed) {
                match check_and_install() {
                    Ok(Some(version)) => {
                        println!(
                            "update: {version} installed, restarting once the window is closed"
                        );
                        crate::wake(crate::Wake::Updated);
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("update: {e}"),
                }
            }
            std::thread::sleep(EVERY);
        }
    });
}

/// The new version installed, or `None` when this one is the latest.
fn check_and_install() -> Result<Option<String>, String> {
    let bundle = own_bundle().ok_or("not running from an app bundle")?;
    let latest = latest_version()?;
    if !newer(&latest, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let work = std::env::temp_dir().join(format!("copycopy-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    let dmg = work.join(ASSET);
    let url = format!("{REPO}/releases/download/v{latest}/{ASSET}");
    let result = run(
        "curl",
        &["-fsSL", "--max-time", "600", "-o", path(&dmg), &url],
    )
    .and_then(|_| install(&dmg, &bundle));
    let _ = std::fs::remove_dir_all(&work);
    result.map(|()| Some(latest))
}

/// `releases/latest` redirects to `releases/tag/vX.Y.Z`: following it gives
/// the version without the API, its JSON or its rate limit.
fn latest_version() -> Result<String, String> {
    let url = run(
        "curl",
        &[
            "-fsSIL",
            "--max-time",
            "30",
            "-o",
            "/dev/null",
            "-w",
            "%{url_effective}",
            &format!("{REPO}/releases/latest"),
        ],
    )?;
    version_from_url(&url).ok_or_else(|| format!("no version in {url}"))
}

fn version_from_url(url: &str) -> Option<String> {
    let tag = url.trim().rsplit_once("/tag/")?.1;
    let version = tag.strip_prefix('v').unwrap_or(tag);
    parse(version).map(|_| version.to_string())
}

fn parse(version: &str) -> Option<Vec<u64>> {
    version.split('.').map(|n| n.parse().ok()).collect()
}

/// `a` is a later version than `b`. Anything unreadable is not newer: a
/// strange tag must never trigger an install.
fn newer(a: &str, b: &str) -> bool {
    matches!((parse(a), parse(b)), (Some(a), Some(b)) if a > b)
}

/// Swaps `bundle` for the app inside `dmg`, if it is signed like `bundle`.
/// Also reached from `--update-from`, to check all this on a local image.
pub fn install(dmg: &Path, bundle: &Path) -> Result<(), String> {
    // Read before anything moves: this is the certificate the new app must carry.
    let requirement = designated_requirement(bundle)?;
    let parent = bundle.parent().ok_or("app bundle has no folder")?;
    // Beside the bundle, so the final swap is a rename on one volume.
    let staged = parent.join(".copycopy-update.app");
    let old = parent.join(".copycopy-old.app");
    let _ = std::fs::remove_dir_all(&staged);
    let _ = std::fs::remove_dir_all(&old);

    let mount = dmg.with_extension("mount");
    run(
        "hdiutil",
        &[
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-quiet",
            "-mountpoint",
            path(&mount),
            path(dmg),
        ],
    )?;
    let copied = run("ditto", &[path(&mount.join("copycopy.app")), path(&staged)]);
    let _ = run("hdiutil", &["detach", "-quiet", path(&mount)]);
    copied?;

    let test = format!("--test-requirement=={requirement}");
    if let Err(e) = run(
        "codesign",
        &["--verify", "--deep", "--strict", &test, path(&staged)],
    ) {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(format!(
            "the download is not signed like this app, not installed ({e})"
        ));
    }

    // The running process keeps its binary: macOS maps it, a rename does not
    // pull it away.
    std::fs::rename(bundle, &old).map_err(|e| format!("{}: {e}", bundle.display()))?;
    if let Err(e) = std::fs::rename(&staged, bundle) {
        let _ = std::fs::rename(&old, bundle);
        return Err(format!("{}: {e}", bundle.display()));
    }
    let _ = std::fs::remove_dir_all(&old);
    Ok(())
}

/// What `codesign` prints as `designated => …`. An ad-hoc signature prints it
/// commented out (`# designated => cdhash …`): there is no certificate to
/// match then, so no update can be trusted.
fn designated_requirement(bundle: &Path) -> Result<String, String> {
    let out = run("codesign", &["-d", "-r-", path(bundle)])?;
    out.lines()
        .find_map(|l| l.strip_prefix("designated => "))
        .map(str::to_string)
        .ok_or_else(|| {
            "this app is not signed with a certificate (a local build?): it cannot update itself"
                .to_string()
        })
}

/// Ends this process and opens `bundle` again, as a resident without window.
/// The shell waits for this process to be gone: started any sooner, the new
/// one would find the IPC taken and only forward a `--show` to us.
pub fn relaunch(bundle: &Path) {
    let script = format!(
        "while kill -0 {} 2>/dev/null; do sleep 0.2; done; open \"$0\" --args --hidden",
        std::process::id()
    );
    if let Err(e) = Command::new("/bin/sh")
        .args(["-c", &script])
        .arg(bundle)
        .spawn()
    {
        eprintln!("update: could not relaunch ({e}), start copycopy again by hand");
    }
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every path here comes from `current_exe` or the temp folder, which macOS
/// keeps in UTF-8.
fn path(p: &Path) -> &str {
    p.to_str().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_from_the_redirect() {
        assert_eq!(
            version_from_url("https://github.com/te-o-o-o/copycopy/releases/tag/v0.6.1\n")
                .as_deref(),
            Some("0.6.1")
        );
        // No release yet: GitHub lands on the releases page itself.
        assert_eq!(
            version_from_url("https://github.com/te-o-o-o/copycopy/releases"),
            None
        );
        assert_eq!(
            version_from_url("https://github.com/x/y/releases/tag/nightly"),
            None
        );
    }

    #[test]
    fn compares_numbers_not_text() {
        assert!(newer("0.10.0", "0.9.3"));
        assert!(newer("1.0.0", "0.6.0"));
        assert!(!newer("0.6.0", "0.6.0"));
        assert!(!newer("0.5.9", "0.6.0"));
        assert!(!newer("garbage", "0.6.0"));
    }
}
