# Security

Please report a vulnerability privately through GitHub:
**Security → Report a vulnerability** on this repository. Do not open a public
issue for it.

The plugin talks only to Herdr's local socket, makes no network calls and
sends nothing anywhere, so the most likely issues are in how it reads local
state (the session snapshot, `.git/HEAD`, its own state files).
