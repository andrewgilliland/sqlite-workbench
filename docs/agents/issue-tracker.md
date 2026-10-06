# Issue tracker: GitHub

Issues and specs live in GitHub Issues for `andrewgilliland/sqlite-workbench`.
Use the `gh` CLI for operations. Run it inside this clone so it infers the
repository from the Git remote, or pass `--repo andrewgilliland/sqlite-workbench`.

## Conventions

- **Create:** `gh issue create --title "..." --body "..."`. Use a heredoc for
  multiline bodies.
- **Read:** `gh issue view <number> --comments`; also fetch labels when relevant
  with `gh issue view <number> --json number,title,body,labels,comments`.
- **List:** `gh issue list --state open --json number,title,body,labels,comments`.
  Apply appropriate `--label`, `--state`, and `--jq` filters.
- **Comment:** `gh issue comment <number> --body "..."`.
- **Apply/remove labels:** `gh issue edit <number> --add-label "..."` or
  `--remove-label "..."`. Use [the triage mapping](triage-labels.md).
- **Close:** `gh issue close <number> --comment "..."`.

When a skill says **publish to the issue tracker**, create a GitHub issue.
When it says **fetch the relevant ticket**, read the issue and its comments.

## Pull requests as a triage surface

**PRs as a request surface: no.** Set to `yes` if external PRs should enter the
triage queue. While disabled, triage applies to issues only.

If enabled, use `gh pr view <number> --comments` and `gh pr diff <number>`;
comment, label, and close with `gh pr comment`, `gh pr edit`, and `gh pr close`.
List candidates with `gh pr list --state open --json
number,title,body,labels,author,authorAssociation,comments` and retain authors
with association `CONTRIBUTOR`, `FIRST_TIME_CONTRIBUTOR`, or `NONE`.

GitHub shares issue and PR numbers. Resolve an ambiguous reference using
`gh pr view <number>` and fall back to `gh issue view <number>`.

## Wayfinding operations

- **Map:** one issue labeled `wayfinder:map`, holding Notes, Decisions-so-far,
  and Fog.
- **Child ticket:** link the ticket as a GitHub sub-issue using `gh api`. Where
  sub-issues are unavailable, list it in the map's task list and put
  `Part of #<map>` at the top of the ticket. Use `wayfinder:<type>` labels:
  `research`, `prototype`, `grilling`, or `task`.
- **Blocking:** use native issue dependencies where available. Add an edge with
  `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by
  -F issue_id=<blocker-db-id>`. Obtain the numeric database ID with
  `gh api repos/<owner>/<repo>/issues/<number> --jq .id`, not the issue number or
  node ID. Otherwise record `Blocked by: #<number>, ...` at the ticket's top.
- **Frontier:** consider the map's open children in map order; skip assigned
  tickets and tickets with open blockers. Use
  `issue_dependencies_summary.blocked_by` where available, otherwise check the
  recorded blocker references. A ticket is unblocked when all blockers close.
- **Claim:** `gh issue edit <number> --add-assignee @me`, the driving session's
  first write.
- **Resolve:** comment with the answer, close the child, and append a gist and
  link to the map's Decisions-so-far.