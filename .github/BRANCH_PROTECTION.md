# Main branch protection

Apply a repository ruleset to `main` with:

- pull requests required, including one approval and dismissal of stale approvals;
- code-owner review required;
- conversation resolution required;
- force pushes and branch deletion blocked;
- branch required to be up to date before merge;
- required status checks:
  - `Test and build`
  - `Full-history secret scan`
  - `CodeQL`

The checks are intentionally secret-free so pull requests from forks can run
without access to repository or deployment credentials. Production deployment
is owned outside this public repository.
