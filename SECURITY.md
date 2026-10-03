# Security Policy (安全策略)

The Gihomo team takes security seriously. As a desktop networking client interacting with proxy configurations, kernel processes, and Linux system networking, we appreciate your efforts to responsibly disclose any vulnerabilities.

## Supported Versions

Only the latest release receives active security updates and patches.

| Version | Supported          |
| :---    | :---:              |
| 1.1.x   | :white_check_mark: |
| < 1.1.0 | :x:                |

## Reporting a Vulnerability

Please **DO NOT** report security vulnerabilities via public GitHub issues, discussions, or pull requests.

Instead, please report security concerns via one of the following private channels:

1. **GitHub Private Vulnerability Reporting** (Preferred):
   Submit a private advisory via the **Security** tab of this repository.
2. **Email**:
   Send an email directly to `zhanmq.china@gmail.com` with:
   - Type of issue (e.g. privilege escalation, secret leak, memory safety).
   - Step-by-step instructions to reproduce the vulnerability.
   - Proof-of-concept (PoC) code or configuration, if applicable.
   - Affected Gihomo version and Linux distribution.

## Security Architecture Principles in Gihomo

For your reference when assessing potential security issues:
- **Local Loopback Controller**: The embedded Mihomo external controller is strictly bound to `127.0.0.1` with a random, private per-user secret stored in `$XDG_DATA_HOME/art.artforge.Gihomo/secret` (file permissions `0600`).
- **Least Privilege (TUN Mode)**: Gihomo does not run the GUI client as `root`. TUN permissions are granted specifically to the Mihomo kernel binary via Linux file capabilities (`CAP_NET_ADMIN` and `CAP_NET_BIND_SERVICE`).
- **Polkit Authorization**: Access to `systemd-resolved` D-Bus interfaces is mediated by FreeDesktop Polkit rules (`/usr/share/polkit-1/rules.d/art.artforge.Gihomo.rules`).

We will acknowledge receipt within 48 hours and work with you on a timely resolution and disclosure schedule.
