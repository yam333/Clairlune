# Distribution: installers and package managers

Decided 2026-11-20: Clairlune ships through package managers. Every package
manager downloads the installer from a public link, so this waited for the
repository to go public (2026-11-20). Everything up to publishing runs on
each release; the first published release starts the channels below.

## What each release builds (`.github/workflows/release.yml`)

| File | For |
|---|---|
| `clairlune_<v>_x64-setup.exe` | Windows: per-user NSIS installer (no admin rights); winget |
| `clairlune_<v>_x64-portable.zip` | Windows: the exe and licences; Scoop |
| `Clairlune_<v>_aarch64.dmg`, `Clairlune_<v>_x64.dmg` | macOS 13+, Apple Silicon and Intel; Homebrew |
| `clairlune_<v>_winget.zip` | The winget manifests, for `winget install --manifest` until the package is in winget's catalog |

The `manifests` job then runs `scripts/package_manifests.py`, which hashes
those files and writes every manifest below into the `clairlune-manifests`
artifact. To make them by hand from downloaded installers:

```sh
python scripts/package_manifests.py --artifacts <folder> --version 1.2.3
```

## Status (0.29.0, 2026-11-23)

| Channel | State |
|---|---|
| GitHub release | live: https://github.com/yam333/Clairlune/releases/tag/v0.29.0 |
| crates.io | live: `cargo install clairlune` |
| Homebrew, Scoop | live in the tap https://github.com/yam333/homebrew-clairlune, updated by publish.yml on each release |
| winget | submitted: https://github.com/microsoft/winget-pkgs/pull/485182, carrying 0.12.0 (validation passed; waits for a moderator); 0.29.0 follows as an update once it is in |

For the next releases, publishing the GitHub release runs `.github/workflows/publish.yml`. It updates the tap through the `TAP_DEPLOY_KEY` secret (set 2026-11-21): the private half of a deploy key on yam333/homebrew-clairlune with write access to that repository and nothing else. It could open the winget update with a `WINGET_TOKEN` secret, but that would have to be a classic token with `public_repo` (write access to every public repository of the account), so it is left unset: each release's winget pull request is opened by hand from the fork yam333/winget-pkgs, from the `clairlune-manifests` artifact.

## Channels

Windows and macOS only: the Linux packages (`.deb`, AppImage, AUR) stopped
with 0.19.0, when the Linux app was dropped (docs/dev/platform-support.md).

| Channel | Manifest | Publishing, once public | Needs |
|---|---|---|---|
| Homebrew (macOS) | `homebrew/Casks/clairlune.rb` | Push to a tap repository named `yam333/homebrew-clairlune`. Users run `brew install --cask yam333/clairlune/clairlune` (recent Homebrew versions ask them to trust a third-party tap once). | A public tap repository. The main `homebrew/cask` repository only takes signed, notarized apps with some following, so the tap comes first. |
| winget (Windows) | `winget/manifests/c/Clairlune/Clairlune/<v>/` | A pull request to `microsoft/winget-pkgs` (by hand with `komac submit`, or from the workflow with a token). Microsoft validates the installer automatically; then `winget install Clairlune.Clairlune`. | A GitHub token for the pull request. Unsigned installers pass but may get extra checks. |
| Scoop (Windows) | `scoop/clairlune.json` | Commit to a bucket repository (the tap repository can hold it too, under `bucket/`). Users run `scoop bucket add clairlune <repo>` then `scoop install clairlune`. Later: propose to the `extras` bucket. | A public bucket repository. |
| Cargo | crates.io | Already live: `cargo install clairlune` (the app: Windows and macOS) and `cargo install clairlune-cli` (any system). Both need Rust. | Nothing more. |

Later, each with its own work first:

- **Chocolatey** (Windows): a `.nuspec` around the setup.exe, a community account, and their moderation queue.
- **Nix / nixpkgs**: a derivation building from the crates.io sources.

## Starting the channels (the first public release)

1. Create `yam333/homebrew-clairlune` (public), with `Casks/` and `bucket/`.
2. Add a `publish` job to the Release workflow that copies the generated cask and Scoop manifest into it on each tag, and opens the winget pull request with `komac` (repository secrets: a token with access to the tap and to a winget-pkgs fork).
3. Publish the draft GitHub release, then run the job (or push the files by hand the first time and check each install command on a clean machine).
4. Put the install commands in the README.

Code signing (an Apple Developer ID with notarization, a Windows code-signing
certificate) makes each of these smoother but isn't required for the tap,
the bucket or winget. There is none (the maintainer's decision), but
the Mac app is still signed ad hoc in the Release workflow: that seals the
bundle, without which an Apple silicon Mac reports a downloaded app as
damaged instead of offering Open Anyway (0.13.0's first DMGs had this).
