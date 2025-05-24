## Pull Request

### Description
Brief description of what this PR does.

### Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Refactoring (no functional changes)
- [ ] Performance improvement
- [ ] Security fix

### Related Issues
Fixes #(issue number)
Related to #(issue number)

### Component Changes
Which components are affected by this change?
- [ ] domainservd
- [ ] publisherd  
- [ ] oxiadm
- [ ] Database schema
- [ ] ActivityPub protocol
- [ ] HTTP signatures
- [ ] Federation logic
- [ ] Message queue
- [ ] Documentation

### Testing
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] I have added tests for edge cases
- [ ] All new and existing tests pass locally
- [ ] I have tested federation compatibility (if applicable)

### Federation Testing
If this affects ActivityPub federation:
- [ ] Tested with Mastodon
- [ ] Tested with Pleroma
- [ ] Tested cross-platform object delivery
- [ ] Verified HTTP signature compliance
- [ ] Tested WebFinger discovery

### Documentation
- [ ] I have updated relevant documentation
- [ ] I have added rustdoc comments for new public APIs
- [ ] I have updated DESIGN.md (if architecture changed)
- [ ] I have updated ARCHITECTURE.md (if implementation details changed)
- [ ] I have added/updated CLI help text (if oxiadm changed)

### Database Changes
- [ ] No database changes
- [ ] Schema additions (backward compatible)
- [ ] Schema modifications (migration needed)
- [ ] New indexes added
- [ ] Data migration script provided

### Breaking Changes
- [ ] This change introduces breaking changes
- [ ] Migration guide provided
- [ ] Deprecation warnings added
- [ ] Backward compatibility maintained

If breaking changes, describe the impact and migration path:

### Security Considerations
- [ ] This change has no security implications
- [ ] This change improves security
- [ ] This change has been reviewed for security implications
- [ ] Security team review requested

### Performance Impact
- [ ] No performance impact
- [ ] Performance improvement
- [ ] Potential performance regression (explained below)
- [ ] Performance testing completed

### Checklist
- [ ] My code follows the project's style guidelines
- [ ] I have performed a self-review of my own code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] My changes generate no new warnings
- [ ] I have run `cargo fmt` and `cargo clippy`
- [ ] I have run the test suite and all tests pass
- [ ] Any dependent changes have been merged and published

### Screenshots (if applicable)
Add screenshots to help explain your changes.

### Additional Notes
Add any other notes about the PR here.