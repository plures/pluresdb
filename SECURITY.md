# Security Policy

## Reporting a Vulnerability

The PluresDB team takes security issues seriously. We appreciate your efforts to responsibly disclose your findings.

### How to Report

If you discover a security vulnerability, please report it by:

1. **Do NOT** open a public GitHub issue
2. Email details to: security@plures.dev (or use GitHub's private vulnerability reporting feature)
3. Include the following information:
   - Description of the vulnerability
   - Steps to reproduce the issue
   - Potential impact
   - Any suggested fixes (optional)

### What to Expect

- **Acknowledgment**: We will acknowledge receipt of your vulnerability report within 48 hours
- **Updates**: We will provide regular updates on our progress
- **Timeline**: We aim to release a fix within 30 days for critical issues
- **Credit**: We will credit you in the security advisory (unless you prefer to remain anonymous)

## Supported Versions

We currently support security updates for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 1.x.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Security Best Practices

When using PluresDB, we recommend:

1. **Keep PluresDB Updated**: Always use the latest stable version
2. **Secure Your Keys**: Protect private keys used for P2P encryption
3. **Network Security**: Use TLS/SSL for network communications in production
4. **Access Control**: Implement proper access controls for your data
5. **Audit Logs**: Enable and regularly review audit logs
6. **Data Encryption**: Use encryption at rest for sensitive data

## Known Security Considerations

### P2P Network Security

- PluresDB uses end-to-end encryption for P2P data sharing
- Peer authentication is based on public key infrastructure
- Always verify peer identities before sharing sensitive data

### Local Data Storage

- Data is stored locally on your device
- Encryption at rest is available but must be explicitly enabled
- Ensure proper file system permissions on data directories

### Web UI Access

- The Web UI runs on localhost by default (port 34568)
- Do not expose the Web UI port to untrusted networks without proper authentication
- Use reverse proxy with authentication for remote access

## Security Updates

Security updates will be announced through:

- GitHub Security Advisories
- Repository CHANGELOG.md
- GitHub Discussions (Security category)

## Contact

For security-related questions that are not vulnerabilities, you can:

- Open a discussion in the Security category: https://github.com/plures/pluresdb/discussions
- Contact us via GitHub issues (for non-sensitive questions only)

Thank you for helping keep PluresDB and its users safe!
