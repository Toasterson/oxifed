---
name: Bug Report
about: Create a report to help us improve Oxifed
title: '[BUG] '
labels: 'bug'
assignees: 'Toasterson'

---

## Bug Description
A clear and concise description of what the bug is.

## To Reproduce
Steps to reproduce the behavior:
1. Go to '...'
2. Click on '....'
3. Scroll down to '....'
4. See error

## Expected Behavior
A clear and concise description of what you expected to happen.

## Actual Behavior
A clear and concise description of what actually happened.

## Environment
- **OS**: [e.g. Ubuntu 22.04, macOS 13.0, Windows 11]
- **Rust Version**: [e.g. 1.75.0]
- **Oxifed Version**: [e.g. 0.1.0, commit hash]
- **Database**: [e.g. MongoDB 6.0.8]
- **Message Queue**: [e.g. RabbitMQ 3.11.0]

## Component
Which component is affected?
- [ ] domainservd
- [ ] publisherd
- [ ] oxiadm
- [ ] Database layer
- [ ] ActivityPub federation
- [ ] HTTP signatures
- [ ] Other (specify):

## Logs
Please include relevant log output:

```
Paste logs here
```

## Configuration
- **Domain configuration**: [if relevant]
- **Federation setup**: [if relevant]
- **Custom configuration**: [if any]

## Additional Context
Add any other context about the problem here, including:
- Screenshots (if applicable)
- Network topology (for federation issues)
- Related issues or PRs
- Workarounds you've tried

## Minimal Reproduction
If possible, provide a minimal example that reproduces the issue:

```rust
// Minimal code example
```

Or steps with oxiadm:
```bash
# Commands that reproduce the issue
```

## Federation Context
If this is a federation-related bug:
- **Remote server software**: [e.g. Mastodon 4.2.0, Pleroma 2.5.0]
- **ActivityPub object type**: [e.g. Note, Article, Follow]
- **HTTP signature validation**: [working/failing]
