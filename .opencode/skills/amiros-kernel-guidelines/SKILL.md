---
name: amiros-kernel-guidelines
description: Core behavioral and technical principles for the AmirOS kernel project.
license: MIT
---

# AmirOS Kernel Development Guidelines

These guidelines ensure that **AmirOS** remains a safe, maintainable, and high-performance Rust-based operating system. Use these when writing, reviewing, or refactoring kernel code.

## 1. Balanced Modularity
**Keep AmirOS lean but organized.**

*   Structure code into distinct modules to reduce binary bloat and simplify maintenance.
*   **The "Anti-Clutter" Rule:** Avoid excessive fragmentation. If a module is too small or creates unnecessary overhead, integrate it into a parent module.
*   Aim for clear interfaces between kernel subsystems (e.g., memory management, scheduling, drivers).

## 2. Documentation-First Approach
**Verify the architecture before the implementation.**

*   **Search First:** Always consult the existing AmirOS documentation and architectural specs before writing code.
*   **Ask for Clarity:** If an architectural decision or code improvement is ambiguous, ask for confirmation before proceeding.
*   **Performance Alignment:** Ensure your implementation reflects documented speed and efficiency requirements.

## 3. Mandatory Linting & Formatting
**Zero-tolerance for warnings in the AmirOS codebase.**

Before submitting any code, you must execute the following cycle:
1.  **Format:** Run `cargo fmt --all -- --check`.
2.  **Lint:** Run `cargo clippy -- -D warnings`.
3.  **The Fix Loop:** If any errors or warnings occur, fix them immediately and re-run the checks.
4.  **Completion:** Code is only ready when both commands pass with zero output.

## 4. Atomic & Sequential Commits
**Maintain a clean and readable project history.**

*   **One Task, One Commit:** Every bugfix, feature, or optimization must be isolated in its own commit.
*   **Order Matters:** If a PR contains multiple optimizations (e.g., 3 or 4 distinct changes), commit them sequentially in the required logical order.
*   **Consistency:** Use a uniform commit message style for every file change across the project.

## 5. Minimal Unsafe Code
**Prioritize Rust's safety guarantees.**

*   **Safe by Default:** Use safe Rust wherever possible.
*   **The Unsafe Exception:** `unsafe` is only permissible when safe code cannot solve the issue due to hardware constraints, correctness, or critical performance needs.
*   **Verification:** If you use `unsafe`, ensure it is the most minimal implementation possible.

## 6. Surgical Precision
**Do not overreach.**

*   **Direct Requests Only:** Do not modify, "improve," or refactor any code or files unless explicitly requested.
*   **Style Matching:** When editing existing files, match the local style perfectly.
*   **Orphan Cleanup:** Only remove dead code or unused imports if *your* changes made them redundant.

## 7. Execution Workflow
1.  **Analyze:** Search relevant docs if possible and define the success criteria.
2.  **Plan:** State a brief plan (1. [Step] → verify: [check]).
3.  **Code:** Implement using the modularity and safety rules.
4.  **Verify:** Loop through the Clippy/Fmt cycle until clean.
5.  **Commit:** Organize changes into atomic commits.