# Plans

One file per feature: `NNNN-short-slug.md` (numbered in order of creation). Created by the
`plan-feature` workflow, kept up to date during implementation, never deleted (history).

## Template

```markdown
# NNNN: <title>

Status: draft | approved | in progress | done

## Goal
What the user gets, in 1–3 sentences.

## Approach
Key technical decisions, APIs used, why. Link ADRs if a decision is architectural.

## Steps
- [ ] 1. … (each step small enough for one commit, ends in a green `verify`)

## Automated tests
What gets unit-tested in rustzoom-core, and how.

## Manual test on Windows
Numbered steps the user performs (desktop and in-game), with expected result.

## Open questions
```
