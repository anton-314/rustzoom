---
name: implement
description: Implement an approved plan from docs/plans/ step by step, verifying and committing after every step.
argument-hint: <plan file or number>
---

# Implement an approved plan

1. Open the plan. If its status is not `approved`, stop and run `/plan-feature` instead.
2. Create a branch `feat/<slug>` (or `fix/<slug>`) from an up-to-date `main`. Set the plan status to `in progress`.
3. For each unchecked step:
   1. Implement only that step. Pure logic → `rustzoom-core` with unit tests written alongside.
      Windows code → `crates/rustzoom`, behind `#[cfg(windows)]`, every `unsafe` block with a `// SAFETY:` comment.
   2. Run the `/verify` workflow. Fix until green. Do not weaken lints or delete tests to get green.
   3. Tick the step in the plan file and commit code + plan together.
      Commit message: Conventional Commits (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `ci:`, `chore:`),
      English, imperative. **No Co-Authored-By / "Generated with" lines, ever.**
4. If reality diverges from the plan (API doesn't work as assumed, better approach found), stop,
   update the plan and tell the user before continuing.
5. When all steps are done: give the user the manual Windows test checklist from the plan and wait for
   their results. Fix reported problems as additional steps.
6. After the user confirms the manual test: set status `done`, push the branch, open a PR with `gh pr create`
   (description without AI attribution). Merge only when the user says so.
