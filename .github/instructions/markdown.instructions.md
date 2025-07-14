# Copilot Instructions (Repository) - Markdown

## coding-markdown

**md-accessibility:** Always provide meaningful alt text for images. Use descriptive link text instead of "click here"
or "read more". Ensure heading hierarchy is logical (don't skip levels). Use semantic markup
for better screen reader compatibility.

**md-code-blocks:** Always specify language for code blocks (```rust, ```bash, etc.) for proper syntax highlighting.
Use inline code (`backticks`) for short code snippets, variables, and file paths. Use indented
code blocks only when language specification isn't needed.

**md-document-structure:** Start documents with a single H1 heading. Use logical heading hierarchy (H1 > H2 > H3).
Include a table of contents for long documents. End with consistent footer information
(license, contributing, etc.). Use front matter for metadata when appropriate.

**md-emphasis:** Use **bold** for strong emphasis and *italic* for light emphasis. Use `code` for technical
terms, file names, and variable names. Avoid using ALL CAPS for emphasis. Use > blockquotes
for citations and important notes. Use --- for horizontal rules sparingly.

**md-front-matter:** Use YAML front matter for document metadata: title, description, author, date, tags.
Keep front matter consistent across similar document types. Use lowercase keys with
hyphens for multi-word keys (creation-date, last-modified).

**md-headings:** Use ATX-style headings (# ## ###) instead of Setext-style (=== ---). Add blank lines
before and after headings. Use sentence case for headings unless proper nouns or technical
terms require capitalization. Don't skip heading levels (H1 to H3 without H2).

**md-images:** Always include meaningful alt text: ![Alt text](image.png). Use relative paths for local
images. Consider image size and loading time. Use HTML img tags when you need to specify
dimensions: `<img src="image.png" alt="Description" width="300">`. Store images in
consistent directory structure.

**md-line-length:** Keep lines under 100 characters when possible. Exceptions: long URLs, code blocks,
tables, and reference-style links. Use proper formatting for lists, headings, and
code blocks. Break long sentences at natural points (after commas, before conjunctions).

**md-links:** Use inline links `[text](url)` for short URLs. Use reference-style links `[text][ref]`
for repeated or long URLs. Use descriptive link text that makes sense out of context.
Validate that all links work. Use relative links for internal documents. Include
meaningful titles: `[text](url "title")`.

**md-lists:** Use `-` for unordered lists consistently. Use `1.` for ordered lists (numbers will
auto-increment). Indent nested lists with 2 spaces. Add blank lines before and after
lists. For multi-paragraph list items, indent continuation paragraphs to align with
the list item text.

**md-mermaid:** - Syntax and Parsing:
  - Quote labels containing special characters: `()`, `[]`, `{}`, `:`, `;`, `#`, `&`
  - Use consistent node ID naming: camelCase or snake_case, no spaces
  - Escape special characters in text with quotes or HTML entities
- Diagram Organization:
  - Start with diagram direction: `graph TD` (top-down), `graph LR` (left-right)
  - Add title with `---` and `title: Your Diagram Title`
  - Keep diagrams under 20 nodes - break complex diagrams into multiple linked diagrams
  - Group related elements using subgraphs with meaningful names
- Styling and Consistency:
  - Define CSS classes for consistent styling: `classDef primary fill:#e1f5fe`
  - Use semantic colors: red for errors, green for success, blue for processes
  - Apply consistent styling: `class nodeId primary`
  - Use meaningful link labels and consistent arrow types
- Best Practices:
  - Choose appropriate diagram type: flowchart for processes, sequence for interactions,
    gitgraph for branching, timeline for chronological events
  - Use descriptive node names that match your domain language
  - Add explanatory text before diagrams describing their purpose and scope
  - Link diagrams to relevant code/documentation with comments
  - Test diagrams in mermaid.live before committing
  - Use `%%` for comments within diagrams to explain complex logic

**md-tables:** Align table columns for readability in source. Use header row with separator line.
Use alignment indicators in separator (`:---`, `:---:`, `---:`). Keep table content
concise - consider breaking large tables into multiple smaller ones. Add blank lines
before and after tables.

**md-whitespace:** Use single blank line to separate paragraphs. Use blank lines before and after headings,
lists, code blocks, and tables. Remove trailing whitespace. End files with single newline.
Use two trailing spaces for line breaks only when necessary - prefer paragraph breaks.


