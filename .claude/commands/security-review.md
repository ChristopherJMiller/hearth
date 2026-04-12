---
description: Run a security review on staged/unstaged changes or a specific file, mapping findings to OWASP/CIS/NIST/STIG. Optional argument is a file path or `--framework cis|stig|nist` to emphasize a particular control framework.
argument-hint: "[file-path] [--framework cis|stig|nist]"
---

Run the `security-review` agent on the current working tree changes.

## Scope

If `$ARGUMENTS` contains a file path, review that file in isolation.
Otherwise, identify changes via:

1. Run `git diff --cached --name-only` to list staged files.
2. Run `git diff --name-only` to list unstaged changes.
3. If both are empty, run `git diff origin/main...HEAD --name-only` to
   review the branch's full diff against `main`.

Read every changed file in full (not just the diff — context matters for
security review).

## Review

Spawn the `security-review` subagent with a prompt containing:
- The full list of changed files, grouped by layer (Rust, Nix, Helm,
  TypeScript, SQL)
- The actual diff content for each file (use `git diff` with the
  appropriate base)
- Any framework emphasis from `--framework`
- Instructions to read related files (auth module, route registrations,
  existing patterns) as needed to verify findings

The subagent will produce a structured report. Your job is to:

1. Present the findings to the user in order of severity
2. For each Critical / High finding, include the suggested remediation
3. Offer to implement the fixes if the user wants you to
4. Summarize which files are clean and which need attention

## Framework focus

If `--framework cis` is passed, emphasize findings that map to CIS
controls and note which controls the current change might regress. Same
pattern for `--framework stig` (DISA STIG) and `--framework nist` (NIST
800-53).

## No findings

If the subagent reports no findings, state that clearly and briefly
describe what was reviewed so the user knows it was not a no-op.
