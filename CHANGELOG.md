# Changelog

All notable changes to Oxifed will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project setup with workspace architecture
- Core daemon infrastructure (domainservd, publisherd)
- ActivityPub protocol implementation
- HTTP signature authentication and validation
- Multi-domain support with isolated configurations
- RabbitMQ message queue integration with hybrid architecture
- MongoDB database layer with proper indexing
- CLI administration tool (oxiadm) for domain and profile management
- WebFinger protocol support for actor discovery
- Public Key Infrastructure (PKI) with trust hierarchy
- Docker containerization and development environment
- Comprehensive test suite with federation testing
- CI/CD pipeline with security auditing
- Complete documentation (DESIGN.md, ARCHITECTURE.md)

### Changed
- N/A (initial release)

### Deprecated
- N/A (initial release)

### Removed
- N/A (initial release)

### Fixed
- N/A (initial release)

### Security
- HTTP signature implementation for ActivityPub authentication
- PKI-based trust system with domain verification
- Rate limiting and security monitoring capabilities
- Comprehensive security audit pipeline

## [0.1.0] - TBD

### Added
- Initial public release
- Core ActivityPub platform functionality
- Federation with major ActivityPub platforms (Mastodon, Pleroma)
- Multi-application support (microblogging, blogging, portfolio)
- Production-ready deployment configuration
- Complete API documentation
- Community guidelines and contribution framework

---

## Release Process

### Version Numbering
- **Major** (X.0.0): Breaking changes to public APIs or federation protocol
- **Minor** (0.X.0): New features, backward-compatible changes
- **Patch** (0.0.X): Bug fixes, security patches, minor improvements

### Release Types
- **Alpha**: Early development versions (0.1.0-alpha.1)
- **Beta**: Feature-complete pre-releases (0.1.0-beta.1)
- **Release Candidates**: Final testing versions (0.1.0-rc.1)
- **Stable**: Production-ready releases (0.1.0)

### Release Notes Format
Each release includes:
- **Overview**: High-level summary of changes
- **Breaking Changes**: API/protocol changes requiring migration
- **New Features**: Added functionality and capabilities
- **Improvements**: Performance, usability, and developer experience
- **Bug Fixes**: Resolved issues and stability improvements
- **Security**: Security-related fixes and enhancements
- **Federation**: ActivityPub compatibility and interoperability updates
- **Documentation**: Documentation improvements and additions
- **Dependencies**: Updated or new dependencies
- **Migration Guide**: Steps for upgrading from previous versions

### Platform Compatibility
Each release is tested against:
- **Mastodon**: Latest stable and previous major version
- **Pleroma**: Latest stable release
- **PeerTube**: Latest stable release
- **Other ActivityPub platforms**: As available

### Support Policy
- **Current Release**: Full support with regular updates
- **Previous Minor**: Security fixes and critical bug fixes
- **Older Releases**: Security fixes only (case-by-case basis)

### Deprecation Policy
- **Advance Notice**: 1 major version for API changes
- **Federation Changes**: Coordinated with ActivityPub community
- **Migration Support**: Tools and guides provided for transitions