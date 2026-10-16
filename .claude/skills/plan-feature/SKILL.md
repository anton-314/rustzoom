---
name: plan-feature
description: Plan a rustzoom feature or change before any code is written. Produces docs/plans/NNNN-slug.md and asks the user to approve it.
argument-hint: <what should be built>
---

# Plan a feature

Goal: an approved, written plan. **Write no production code in this workflow.**

1. Read `CLAUDE.md`, `docs/adr/`, the existing plans in `docs/plans/` and the code the change touches.
2. If a Windows API is involved, check its real behavior (docs via WebFetch/WebSearch) instead of guessing:
   required Windows version, threading/COM apartment requirements, whether it works over borderless games.
3. Check the change against the anti-cheat rules in ADR 0001. If it would break one, stop and tell the user.
4. Collect open decisions. Ask the user only about what truly changes the plan (AskUserQuestion, max 4);
   decide the rest yourself and write the decision into the plan.
5. Write `docs/plans/NNNN-slug.md` using the template in `docs/plans/README.md`:
   - steps small enough for one commit each, each ending green under `/verify`
   - pure logic goes into `rustzoom-core` and gets unit tests; list them
   - a concrete manual test checklist for Windows (desktop and in-game), with expected results
6. If the plan makes an architectural decision, add a new ADR in `docs/adr/`.
7. Show the user a short summary (in German) and the plan path. Status stays `draft` until the user approves;
   then set it to `approved`.
