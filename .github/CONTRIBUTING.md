# Contributing to Oxifed

We love your input! We want to make contributing to Oxifed as easy and transparent as possible, whether it's:

- Reporting a bug
- Discussing the current state of the code
- Submitting a fix
- Proposing new features
- Becoming a maintainer

## 🚀 Getting Started

### Prerequisites

- Rust 1.70+ with `rustfmt` and `clippy`
- Docker & Docker Compose
- MongoDB 6.0+
- RabbitMQ 3.11+
- Git

### Development Setup

1. **Fork and clone the repository:**
   ```bash
   git clone https://github.com/Toasterson/oxifed.git
   cd oxifed
   ```

2. **Start development infrastructure:**
   ```bash
   docker-compose up -d mongodb lavinmq
   ```

3. **Build the project:**
   ```bash
   cargo build
   ```

4. **Run tests:**
   ```bash
   cargo test --workspace
   ```

5. **Check code quality:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets --all-features -- -D warnings
   ```

## 📋 Development Workflow

### Branch Naming

- `feature/description` - New features
- `fix/description` - Bug fixes
- `docs/description` - Documentation updates
- `refactor/description` - Code refactoring
- `test/description` - Test improvements

### Code Standards

- **Formatting**: Use `cargo fmt` before committing
- **Linting**: Ensure `cargo clippy` passes without warnings
- **Testing**: Write tests for new functionality
- **Documentation**: Document public APIs with rustdoc
- **Commit Messages**: Use conventional commits format

### Conventional Commits

```
type(scope): description

[optional body]

[optional footer(s)]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

**Examples:**
```
feat(activitypub): add support for Article objects
fix(domainservd): resolve actor lookup race condition
docs(api): update ActivityPub endpoint documentation
```

## 🐛 Bug Reports

**Great Bug Reports** tend to have:

- A quick summary and/or background
- Steps to reproduce
  - Be specific!
  - Give sample code if you can
- What you expected would happen
- What actually happens
- Notes (possibly including why you think this might be happening, or stuff you tried that didn't work)

Use our [bug report template](.github/ISSUE_TEMPLATE/bug_report.md) when filing issues.

## 💡 Feature Requests

We track feature requests as GitHub issues. When creating a feature request:

- Use a clear, descriptive title
- Provide detailed description of the proposed feature
- Explain why this feature would be useful
- Consider providing examples of how it would work

Use our [feature request template](.github/ISSUE_TEMPLATE/feature_request.md).

## 🔄 Pull Request Process

1. **Create a feature branch** from `develop`
2. **Make your changes** following our coding standards
3. **Add or update tests** as needed
4. **Update documentation** for any public API changes
5. **Ensure CI passes** - all tests, linting, and security checks
6. **Create a pull request** targeting `develop` branch

### Pull Request Guidelines

- Fill out the pull request template completely
- Link any related issues
- Include screenshots for UI changes
- Keep changes focused and atomic
- Write clear, descriptive commit messages

### Review Process

- At least one maintainer review is required
- All CI checks must pass
- Security audit must pass for dependencies
- Documentation must be updated for public API changes

## 🏗️ Architecture Guidelines

### Code Organization

```
crates/
├── domainservd/     # Domain service daemon
├── publisherd/      # ActivityPub publishing service
├── oxiadm/          # CLI administration tool
├── common/          # Shared libraries
└── activitypub/     # ActivityPub protocol implementation
```

### Key Principles

- **Modularity**: Keep components loosely coupled
- **Testability**: Write testable code with clear interfaces
- **Documentation**: Document public APIs and complex logic
- **Performance**: Consider scalability in design decisions
- **Security**: Follow security best practices

### Database Guidelines

- Use MongoDB collections appropriately (see `ARCHITECTURE.md`)
- Create proper indexes for query patterns
- Handle errors gracefully
- Use transactions for multi-document operations

### ActivityPub Compliance

- Follow [W3C ActivityPub specification](https://www.w3.org/TR/activitypub/)
- Test federation with existing platforms (Mastodon, Pleroma)
- Implement proper HTTP signatures
- Support standard ActivityStreams vocabulary

## 🧪 Testing

### Test Categories

- **Unit Tests**: Test individual functions and modules
- **Integration Tests**: Test component interactions
- **Federation Tests**: Test ActivityPub protocol compliance
- **End-to-End Tests**: Test complete user workflows

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific test suite
cargo test --package domainservd

# Integration tests
cargo test --test integration

# With test output
cargo test -- --nocapture
```

### Test Data

Use the provided test data generators:
```bash
./generate_test_data.sh
```

## 📚 Documentation

### Types of Documentation

- **Code Documentation**: Rustdoc comments for public APIs
- **Architecture Documentation**: High-level system design
- **User Documentation**: Setup and usage guides
- **API Documentation**: REST endpoint specifications

### Writing Documentation

- Use clear, concise language
- Include code examples
- Update documentation with code changes
- Test documentation examples

## 🔒 Security

### Reporting Security Vulnerabilities

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them privately using GitHub's security advisory feature or by contacting the maintainers directly.

### Security Guidelines

- Never commit secrets, keys, or credentials
- Use HTTP signatures for ActivityPub authentication
- Validate all input data
- Follow OWASP security guidelines
- Keep dependencies updated

## 🎯 Performance Guidelines

- Profile code before optimizing
- Use appropriate data structures
- Consider database query efficiency
- Monitor memory usage
- Test under realistic load conditions

## 📦 Dependency Management

- Keep dependencies minimal and well-maintained
- Use workspace dependencies for consistency
- Regular security audits with `cargo audit`
- Document rationale for new dependencies

## 🌍 Community Guidelines

### Code of Conduct

We are committed to providing a welcoming and inspiring community for all. Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

### Communication

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: General questions and community discussion
- **Pull Requests**: Code contributions and reviews

### Getting Help

- Check existing issues and documentation first
- Use GitHub Discussions for questions
- Join our community chat (if available)
- Attend community meetings (if scheduled)

## 🏆 Recognition

Contributors are recognized in several ways:

- Listed in `CONTRIBUTORS.md`
- Mentioned in release notes
- GitHub contributor statistics
- Special recognition for significant contributions

## 📋 Release Process

1. **Version Bump**: Update version in `Cargo.toml`
2. **Changelog**: Update `CHANGELOG.md` with changes
3. **Testing**: Ensure all tests pass
4. **Review**: Get maintainer approval
5. **Release**: Create GitHub release with changelog
6. **Publish**: Publish to crates.io (if applicable)

## 🔄 Maintenance

### Regular Tasks

- Dependency updates
- Security patches
- Performance monitoring
- Documentation updates
- Community engagement

### Long-term Goals

See our [roadmap](DESIGN.md#development-roadmap) for planned features and improvements.

## ❓ Questions?

Don't hesitate to ask questions! You can:

- Open a GitHub Discussion
- Comment on relevant issues
- Reach out to maintainers

Thank you for contributing to Oxifed! 🎉