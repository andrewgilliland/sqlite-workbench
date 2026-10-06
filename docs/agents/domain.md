# Domain docs

## Layout

This is a **single-context** repository:

- `CONTEXT.md` at the repository root contains the domain glossary, when present.
- `docs/adr/` contains repository-wide architectural decisions.

## Before exploring or designing

Read the root glossary if present and ADRs relevant to the work. If the repository
later gains a root `CONTEXT-MAP.md`, follow its pointers and read the glossaries
and context-scoped ADRs relevant to the topic.

If a glossary or ADR directory is absent, proceed silently. Domain modeling
creates these lazily when terminology or decisions are resolved.

## Vocabulary

Use glossary terms in issue titles, tests, hypotheses, and design discussions.
If a needed concept is missing, reconsider whether it belongs to the domain or
note the gap for domain modeling.

Keep the glossary limited to domain vocabulary. Implementation plans and
architectural decisions belong outside it; decisions belong in ADRs.

## ADR conflicts

Surface conflicts with an existing ADR explicitly rather than silently overriding
it. Identify the decision and explain why it may need to be revisited. Respect
each ADR's recorded status: a proposed ADR is a draft, not an accepted decision.
