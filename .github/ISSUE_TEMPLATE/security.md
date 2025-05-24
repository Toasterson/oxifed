---
name: Security Vulnerability
about: Report a security vulnerability in Oxifed
title: '[SECURITY] '
labels: 'security'
assignees: 'Toasterson'

---

## ⚠️ Security Vulnerability Report

**Please do not report security vulnerabilities through public GitHub issues.**

For security vulnerabilities, please report them privately:
- Use GitHub's private vulnerability reporting feature
- Contact the maintainers directly through GitHub

## If this is not a security issue, please use the appropriate template instead.

---

## For Security Team Use Only

### Vulnerability Type
- [ ] Authentication bypass
- [ ] Authorization vulnerability
- [ ] Injection vulnerability
- [ ] Cryptographic issue
- [ ] HTTP signature bypass
- [ ] ActivityPub protocol abuse
- [ ] Information disclosure
- [ ] Denial of Service
- [ ] Other:

### Affected Components
- [ ] domainservd
- [ ] publisherd
- [ ] oxiadm
- [ ] HTTP signature validation
- [ ] ActivityPub federation
- [ ] Database layer
- [ ] Message queue
- [ ] PKI implementation

### Severity Assessment
- [ ] Critical - Remote code execution, full system compromise
- [ ] High - Authentication bypass, privilege escalation
- [ ] Medium - Information disclosure, limited access
- [ ] Low - Minor information leakage, DoS

### CVE Information
- CVE ID (if assigned):
- CVSS Score:
- CWE Classification:

### Disclosure Timeline
- [ ] Immediate disclosure needed
- [ ] Standard 90-day disclosure
- [ ] Extended disclosure period requested
- [ ] Coordinated disclosure with other platforms

### Mitigation Status
- [ ] Patch developed
- [ ] Patch tested
- [ ] Security advisory prepared
- [ ] Coordinating with downstream projects

### Federation Impact
- [ ] Affects federation with other ActivityPub servers
- [ ] Could be exploited across federation network
- [ ] Requires coordinated response with other platforms
- [ ] Platform-specific vulnerability
