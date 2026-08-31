# Issue tracker: GitHub

Issues and PRDs for this repo live as GitHub issues at [SlimeBoyOwO/LingChat](https://github.com/SlimeBoyOwO/LingChat). Use the `gh` CLI for all operations.

## Conventions

- **Create an issue**: `gh issue create --repo SlimeBoyOwO/LingChat --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --repo SlimeBoyOwO/LingChat --comments`, filtering comments by `jq` and also fetching labels.
- **List issues**: `gh issue list --repo SlimeBoyOwO/LingChat --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> --repo SlimeBoyOwO/LingChat --body "..."`
- **Apply / remove labels**: `gh issue edit <number> --repo SlimeBoyOwO/LingChat --add-label "..."` / `--remove-label "..."`
- **Close**: `gh issue close <number> --repo SlimeBoyOwO/LingChat --comment "..."`

Since this working directory may not have a git remote configured, always pass `--repo SlimeBoyOwO/LingChat` explicitly.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --repo SlimeBoyOwO/LingChat --comments`.
