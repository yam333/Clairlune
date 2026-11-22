# Security policy

## Supported versions

Only the latest release gets fixes. The app updates itself, so that's the one
nearly everyone runs.

## Reporting a vulnerability

Please report privately, through GitHub: the repository's **Security** tab,
then **Report a vulnerability**. Don't open a public issue for it.

Say what you found, how to reproduce it and what it affects. You'll get an
answer within a week, and credit in the release notes when it's fixed, unless
you'd rather not.

## What's in scope

- **Updates:** Clairlune downloads its installer from this repository's
  GitHub releases (with the system's `curl`), checks it against the SHA-256
  digest GitHub publishes for the asset, then runs it. Anything that gets a
  different file run is in scope.
- **The second-launch socket:** a second copy of the app asks the running one
  to open Settings over a local named pipe or socket.
- **Files the app reads:** settings, saved scene packages, recipes and the
  timeline in the user's own folders.
- **The command-line tool,** when it sets or restores the wallpaper.

Clairlune makes no other network connections: no telemetry, no accounts.
