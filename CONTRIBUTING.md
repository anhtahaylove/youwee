# Contributing to Youwee

We welcome contributions! Here's how you can help.

## Getting Started

1. Fork the repository
2. Create a feature branch from `main`: `git checkout main && git pull origin main && git checkout -b feature/amazing-feature`
3. Install pre-commit hook (recommended):
   ```bash
   cp scripts/pre-commit .git/hooks/ && chmod +x .git/hooks/pre-commit
   ```
4. Make your changes
5. Run tests and linting:
   ```bash
   bun run lint
   bun run build
   cd src-tauri && cargo check
   ```
6. Commit your changes: `git commit -m 'feat: add amazing feature'`
7. Push to the branch: `git push origin feature/amazing-feature`
8. Open a Pull Request with `main` as the base branch

## Branch and Pull Request Workflow

- Always branch from the latest `main` before starting work.
- Use a focused branch name, such as `feature/browser-extension-docs` or `fix/download-path`.
- Keep your branch up to date with `main` while working if the PR becomes stale.
- When opening a Pull Request, select `main` as the base branch and your feature/fix branch as the compare branch.
- Do not merge directly into `main`; use a Pull Request so changes can be reviewed.

## Commit Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `style:` - Code style changes (formatting, etc.)
- `refactor:` - Code refactoring
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks

## Development Tips

- Run `bun run tauri dev` for hot-reloading development
- Frontend changes reflect immediately
- Rust changes require recompilation (automatic in dev mode)

## Code Style

- Use TypeScript for frontend code
- Follow existing code patterns and conventions
- Use Biome for linting and formatting
- Keep components small and focused

## Pull Request Guidelines

- Keep PRs focused on a single feature or fix
- Write clear commit messages following the convention
- Update documentation if needed
- Test your changes thoroughly before submitting
