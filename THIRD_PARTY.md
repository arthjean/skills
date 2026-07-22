# Third-party notices

This repository bundles skills from several upstream projects. Those files retain their original licenses and copyrights. The top-level MIT license applies only to original material authored by Arthur.

| Upstream project | Included skills |
| --- | --- |
| [better-auth/skills](https://github.com/better-auth/skills) | `better-auth-best-practices`, `better-auth-security-best-practices`, `create-auth`, `email-and-password-best-practices`, `organization-best-practices`, `two-factor-authentication-best-practices` |
| [cursor/plugins](https://github.com/cursor/plugins) | `thermo-nuclear-code-quality-review` |
| [ibelick/ui-skills](https://github.com/ibelick/ui-skills) | `baseline-ui`, `ui-skills-root` |
| [jakubkrehel/skills](https://github.com/jakubkrehel/skills) | `better-colors`, `better-typography`, `better-ui` |
| [jakubkrehel/make-interfaces-feel-better](https://github.com/jakubkrehel/make-interfaces-feel-better) | `make-interfaces-feel-better` |
| [jakubkrehel/oklch-skill](https://github.com/jakubkrehel/oklch-skill) | `oklch-skill` |
| [mattpocock/skills](https://github.com/mattpocock/skills) | `code-review`, `codebase-design`, `diagnosing-bugs`, `domain-modeling`, `grill-me`, `grill-with-docs`, `grilling`, `handoff`, `implement`, `improve-codebase-architecture`, `research`, `wayfinder`, `writing-great-skills` |
| [nextlevelbuilder/ui-ux-pro-max-skill](https://github.com/nextlevelbuilder/ui-ux-pro-max-skill) | `ui-ux-pro-max` |
| [remotion-dev/skills](https://github.com/remotion-dev/skills) | `remotion-best-practices` |
| [vercel-labs/skills](https://github.com/vercel-labs/skills) | `find-skills` |

The detector scripts under `skills/impeccable/scripts/detector/` carry embedded `Apache-2.0` SPDX notices and a 2026 copyright notice for Paul Bakaus.

## Rust dependencies added for the installer transaction engine

| Crate | License | Purpose |
| --- | --- | --- |
| `fs2` | MIT OR Apache-2.0 | Non-blocking inter-process transaction locks |
| `rustix` | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Descriptor-relative filesystem mutations |
| `signal-hook` | Apache-2.0 OR MIT | Async-signal-safe interruption flags |
