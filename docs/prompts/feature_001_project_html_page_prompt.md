@~/.claude/prompts/new_functionality_prompt_spec.md

# Add Project Description HTML Page

## Role
Act as a Software Developer and Frontend Engineer, expert in HTML, CSS, and project documentation presentation.

## Context
- Project: `letsencrypt-client` — a Rust CLI implementing the ACME protocol (RFC 8555) to obtain TLS certificates from Let's Encrypt or Pebble (test environment).
- Project root: `D:\Master-IA-Dev\06-Bloque6\1-6-30-letsencrypt-client`
- The project includes a `test-app/` directory with an Express HTTPS server (`server.js`) that serves on port 8443.
- GitHub URL: to be filled in by user (e.g. `https://github.com/<user>/letsencrypt-client`)
- GitLab URL: to be filled in by user (e.g. `https://gitlab.codecrypto.academy/<user>/letsencrypt-client`)

## Task
Create a single static HTML page (`index.html`) at the project root that describes the `letsencrypt-client` project. The page must include:

1. Project title and short description (ACME client in Rust for TLS certificate automation).
2. Key features list (ECDSA P-256, HTTP-01 challenge, multi-domain SAN, Pebble test environment).
3. Quick-start / usage section showing the CLI commands (`acme-client issue`, `renew`, `show`).
4. Links section with clickable badges/buttons for:
   - GitHub repository URL
   - GitLab repository URL
5. Tech stack badges (Rust, Node.js, Docker, Let's Encrypt).
6. Footer with author info.

### HTML Page Guidelines
- Pure HTML + inline CSS only — no external frameworks, no JS dependencies.
- Responsive layout (mobile-friendly via CSS flexbox/grid).
- Use a clean, minimal dark or light theme consistent with developer tooling aesthetics.
- All links must open in a new tab (`target="_blank" rel="noopener"`).
- Page must be accessible (semantic HTML5 tags: `<header>`, `<main>`, `<section>`, `<footer>`).
- File saved at: `D:\Master-IA-Dev\06-Bloque6\1-6-30-letsencrypt-client\index.html`

## Output format
A single `index.html` file with:
- Valid HTML5 doctype
- Inline `<style>` block in `<head>`
- All content in `<body>` using semantic tags
- No external CDN calls (fully self-contained)

## Examples and Steps to follow

1. **Git**: Create a new local branch `feature/001-project-html-page`.
2. **Create** `index.html` at project root with all required sections.
   - Replace placeholder GitHub URL with the actual repo URL.
   - Replace placeholder GitLab URL with the actual repo URL.
3. **Verify** the page renders correctly by opening it in a browser (use `/run` or open manually).
4. **Test** all links are correct and open in a new tab.
5. **Commit** locally: `git commit -m "feat: add project description HTML page with GitHub and GitLab links"`.
6. **Push** remote branch and create Pull Request using `/git-only-update`.
7. **Merge** PR into main, switch to local main, pull remote changes.
8. **Final check**: open `index.html` from main branch and confirm all content is present.

## Output checklist and Guardrails
- [ ] `index.html` exists at project root
- [ ] Page title matches project name
- [ ] GitHub URL link is present and correct
- [ ] GitLab URL link is present and correct
- [ ] All links open in a new tab with `rel="noopener"`
- [ ] Page is self-contained (no external HTTP requests)
- [ ] Valid HTML5 structure with semantic tags
- [ ] Responsive on mobile viewport
- [ ] No broken references or missing assets
- [ ] Committed and pushed on a feature branch before merging
