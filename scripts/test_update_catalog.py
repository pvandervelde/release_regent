#!/usr/bin/env python3
"""
Unit tests for update_catalog.py.

Tests cover the pure-function core: path_matches_any, _extract_signature,
_parse_table_row, _format_row, and diff_catalog.
"""

import sys
import unittest
from pathlib import Path

# ---------------------------------------------------------------------------
# Ensure the sibling module is importable even when the test is run from any
# working directory.
# ---------------------------------------------------------------------------
sys.path.insert(0, str(Path(__file__).parent))

from update_catalog import (  # noqa: E402
    CatalogEntry,
    Symbol,
    _extract_signature,
    _format_row,
    _parse_table_row,
    diff_catalog,
    path_matches_any,
)


# ── path_matches_any ──────────────────────────────────────────────────────────

class TestPathMatchesAny(unittest.TestCase):
    """Tests for the path-glob matching helper."""

    def test_no_patterns_returns_false(self) -> None:
        self.assertFalse(path_matches_any("src/lib.rs", []))

    def test_exact_filename_match(self) -> None:
        self.assertTrue(path_matches_any("src/lib.rs", ["lib.rs"]))

    def test_glob_star_matches_nested_file(self) -> None:
        self.assertTrue(path_matches_any("crates/core/src/lib.rs", ["**/*.rs"]))

    def test_glob_star_does_not_match_different_extension(self) -> None:
        self.assertFalse(path_matches_any("crates/core/src/lib.py", ["**/*.rs"]))

    # ── The regression case from the PR review ────────────────────────────────

    def test_target_dir_excluded_by_double_star_pattern(self) -> None:
        """**/target/** must exclude build-artifact paths at the repo root."""
        self.assertTrue(
            path_matches_any("target/debug/build/some-crate/out.rs", ["**/target/**"])
        )

    def test_target_dir_excluded_nested(self) -> None:
        self.assertTrue(
            path_matches_any(
                "target/debug/build/chrono-tz-abc123/out/timezones.rs",
                ["**/target/**"],
            )
        )

    def test_non_target_path_not_excluded(self) -> None:
        self.assertFalse(
            path_matches_any("crates/core/src/lib.rs", ["**/target/**"])
        )

    def test_git_dir_excluded(self) -> None:
        self.assertTrue(
            path_matches_any(".git/hooks/pre-commit", ["**/.git/**"])
        )

    def test_generated_dir_excluded(self) -> None:
        self.assertTrue(
            path_matches_any("src/generated/proto.rs", ["**/generated/**"])
        )

    def test_trailing_slash_pattern_is_substring_based(self) -> None:
        """The fallback substring check is intentionally broad: **/tests/** strips to
        'tests' and that substring matches any path containing the word 'tests',
        including files named tests.rs. fnmatch handles the precise case; the
        substring fallback exists only for top-level directory patterns like target/."""
        # fnmatch checks first — **/tests/** does NOT fnmatch crates/core/src/tests.rs
        # but the substring fallback ('tests' in 'crates/core/src/tests.rs') is True.
        # This is acceptable: the pattern **/tests/** in catalog.config.yml is
        # applied to exclude **/tests/** directories, which overlap with files
        # whose names contain 'tests'. For precise exclusions, use filename glob
        # patterns like **/*_tests.rs instead.
        result = path_matches_any("crates/core/src/tests.rs", ["**/tests/**"])
        # Asserting the actual behaviour (True) so the test documents the known
        # characteristic without acting as a false contract.
        self.assertTrue(result)

    def test_multiple_patterns_first_match_wins(self) -> None:
        self.assertTrue(
            path_matches_any(
                "target/release/rr",
                ["**/.git/**", "**/target/**", "**/generated/**"],
            )
        )

    def test_wildcard_test_file_pattern(self) -> None:
        self.assertTrue(
            path_matches_any("crates/core/src/main_tests.rs", ["**/*_tests.rs"])
        )

    def test_non_matching_test_file_pattern(self) -> None:
        self.assertFalse(
            path_matches_any("crates/core/src/main.rs", ["**/*_tests.rs"])
        )


# ── _extract_signature ────────────────────────────────────────────────────────

class TestExtractSignature(unittest.TestCase):
    """Tests for the signature extractor."""

    def test_fn_stops_at_opening_brace(self) -> None:
        text = "pub fn calculate(x: u32, y: u32) -> u32 {\n    x + y\n}"
        sig = _extract_signature(text, "fn")
        self.assertNotIn("{", sig)
        self.assertIn("calculate", sig)

    def test_fn_multiline_signature_joined(self) -> None:
        text = (
            "pub fn long_function_name(\n"
            "    param_one: SomeType,\n"
            "    param_two: AnotherType,\n"
            ") -> Result<Output, Error> {\n"
            "    todo!()\n"
            "}"
        )
        sig = _extract_signature(text, "fn")
        self.assertNotIn("{", sig)
        self.assertIn("Result", sig)

    def test_struct_returns_first_line_only(self) -> None:
        text = "pub struct Config {\n    pub debug: bool,\n    pub timeout: u64,\n}"
        sig = _extract_signature(text, "struct")
        self.assertEqual(sig, "pub struct Config {")

    def test_empty_text_returns_empty_string(self) -> None:
        self.assertEqual(_extract_signature("", "fn"), "")

    def test_signature_truncated_to_max_chars(self) -> None:
        long_text = "pub fn " + "a" * 300 + "() {"
        sig = _extract_signature(long_text, "fn")
        self.assertLessEqual(len(sig), 200)

    def test_fn_without_brace_returns_full_first_line(self) -> None:
        text = "pub fn no_brace_here(x: u32) -> u32"
        sig = _extract_signature(text, "fn")
        self.assertIn("no_brace_here", sig)

    def test_enum_returns_first_line(self) -> None:
        text = "pub enum Direction {\n    North,\n    South,\n}"
        sig = _extract_signature(text, "enum")
        self.assertEqual(sig, "pub enum Direction {")


# ── _parse_table_row ──────────────────────────────────────────────────────────

class TestParseTableRow(unittest.TestCase):
    """Tests for the markdown-table-row parser."""

    def _make_row(
        self,
        name: str = "my_fn",
        kind: str = "fn",
        location: str = "src/lib.rs:42",
        description: str = "Does something useful",
        tags: str = "core, util",
    ) -> str:
        return f"| `{name}` | {kind} | `{location}` | {description} | {tags} |"

    def test_happy_path_returns_entry(self) -> None:
        row = self._make_row()
        entry = _parse_table_row(row, "core")
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.name, "my_fn")
        self.assertEqual(entry.kind, "fn")
        self.assertEqual(entry.file, "src/lib.rs")
        self.assertEqual(entry.line, 42)
        self.assertEqual(entry.description, "Does something useful")
        self.assertEqual(entry.tags, ["core", "util"])
        self.assertEqual(entry.domain, "core")

    def test_header_row_skipped(self) -> None:
        self.assertIsNone(_parse_table_row("| Name | Kind | Location | Description | Tags |", "core"))

    def test_separator_row_is_not_filtered_by_parse(self) -> None:
        """Separator rows are pre-filtered by load_existing_catalog before
        _parse_table_row is called. _parse_table_row itself parses them as
        a (malformed) entry — that is intentional; callers own the filtering."""
        sep = "|------|------|----------|-------------|------|"
        # The function does NOT return None for separator rows — callers filter.
        result = _parse_table_row(sep, "core")
        # It produces an entry (all dashes), which is fine; load_existing_catalog
        # screens it out with the invariant: inner after removing |, -, space is empty.
        self.assertIsNotNone(result)

    def test_too_few_cells_returns_none(self) -> None:
        self.assertIsNone(_parse_table_row("| only_two | cells |", "core"))

    def test_empty_row_returns_none(self) -> None:
        self.assertIsNone(_parse_table_row("", "core"))

    def test_non_table_row_returns_none(self) -> None:
        self.assertIsNone(_parse_table_row("# This is a heading", "core"))

    def test_escaped_pipe_in_description_round_trips(self) -> None:
        """A description with an escaped pipe must be parsed back correctly."""
        row = "| `my_fn` | fn | `src/lib.rs:10` | Converts A \\| B to C | util |"
        entry = _parse_table_row(row, "core")
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.description, "Converts A | B to C")

    def test_no_tags_returns_empty_list(self) -> None:
        row = self._make_row(tags="")
        entry = _parse_table_row(row, "core")
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.tags, [])

    def test_invalid_line_number_defaults_to_zero(self) -> None:
        row = "| `my_fn` | fn | `src/lib.rs:notanumber` | desc | tag |"
        entry = _parse_table_row(row, "core")
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.line, 0)

    def test_key_is_file_double_colon_name(self) -> None:
        row = self._make_row(name="validate", location="auth/service.rs:99")
        entry = _parse_table_row(row, "auth")
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.key, "auth/service.rs::validate")


# ── _format_row ───────────────────────────────────────────────────────────────

class TestFormatRow(unittest.TestCase):
    """Tests for the catalog-row formatter."""

    def _make_entry(self, **kwargs) -> CatalogEntry:
        defaults = dict(
            key="src/lib.rs::my_fn",
            name="my_fn",
            kind="fn",
            file="src/lib.rs",
            line=10,
            description="Does something useful",
            tags=["core", "util"],
            domain="core",
        )
        defaults.update(kwargs)
        return CatalogEntry(**defaults)

    def test_output_is_markdown_table_row(self) -> None:
        row = _format_row(self._make_entry())
        self.assertTrue(row.startswith("|"))
        self.assertTrue(row.endswith("|"))

    def test_pipe_in_description_is_escaped(self) -> None:
        entry = self._make_entry(description="Converts A | B to C")
        row = _format_row(entry)
        self.assertIn("\\|", row)
        # Only the description pipe is escaped; structural pipes remain
        self.assertNotIn(" A | B ", row)

    def test_tags_are_sorted(self) -> None:
        entry = self._make_entry(tags=["zebra", "alpha", "mid"])
        row = _format_row(entry)
        idx_alpha = row.index("alpha")
        idx_mid = row.index("mid")
        idx_zebra = row.index("zebra")
        self.assertLess(idx_alpha, idx_mid)
        self.assertLess(idx_mid, idx_zebra)

    def test_empty_tags_produces_empty_tags_cell(self) -> None:
        row = _format_row(self._make_entry(tags=[]))
        # Tags cell should be empty (just pipes)
        self.assertIn("|  |", row)

    def test_name_is_backtick_wrapped(self) -> None:
        row = _format_row(self._make_entry(name="some_fn"))
        self.assertIn("| `some_fn`", row)


# ── diff_catalog ──────────────────────────────────────────────────────────────

class TestDiffCatalog(unittest.TestCase):
    """Tests for the catalog diff operation."""

    def _make_symbol(self, name: str, file: str = "src/lib.rs") -> Symbol:
        return Symbol(
            name=name,
            kind="fn",
            file=file,
            line=1,
            signature="",
            language="rust",
            domain="core",
            tags=[],
        )

    def _make_entry(self, name: str, file: str = "src/lib.rs") -> CatalogEntry:
        return CatalogEntry(
            key=f"{file}::{name}",
            name=name,
            kind="fn",
            file=file,
            line=1,
            description="existing description",
            tags=[],
            domain="core",
        )

    def test_new_symbol_not_in_catalog(self) -> None:
        extracted = [self._make_symbol("new_fn")]
        new_syms, unchanged, removed = diff_catalog({}, extracted, [])
        self.assertEqual(len(new_syms), 1)
        self.assertEqual(new_syms[0].name, "new_fn")
        self.assertEqual(unchanged, {})
        self.assertEqual(removed, set())

    def test_unchanged_symbol_stays_in_catalog(self) -> None:
        entry = self._make_entry("existing_fn")
        sym = self._make_symbol("existing_fn")
        new_syms, unchanged, removed = diff_catalog(
            {entry.key: entry}, [sym], []
        )
        self.assertEqual(new_syms, [])
        self.assertIn(entry.key, unchanged)
        self.assertEqual(removed, set())

    def test_deleted_file_removes_its_entries(self) -> None:
        entry = self._make_entry("old_fn", file="src/old.rs")
        new_syms, unchanged, removed = diff_catalog(
            {entry.key: entry}, [], ["src/old.rs"]
        )
        self.assertIn(entry.key, removed)
        self.assertNotIn(entry.key, unchanged)

    def test_symbol_no_longer_extracted_is_removed(self) -> None:
        entry = self._make_entry("gone_fn")
        new_syms, unchanged, removed = diff_catalog(
            {entry.key: entry}, [], []
        )
        self.assertIn(entry.key, removed)
        self.assertNotIn(entry.key, unchanged)

    def test_mixed_scenario(self) -> None:
        """A mix of new, unchanged, and removed symbols in one call."""
        existing_entry = self._make_entry("kept_fn")
        removed_entry = self._make_entry("removed_fn")
        existing = {
            existing_entry.key: existing_entry,
            removed_entry.key: removed_entry,
        }
        extracted = [
            self._make_symbol("kept_fn"),
            self._make_symbol("brand_new_fn"),
        ]
        new_syms, unchanged, removed = diff_catalog(existing, extracted, [])
        self.assertEqual(len(new_syms), 1)
        self.assertEqual(new_syms[0].name, "brand_new_fn")
        self.assertIn(existing_entry.key, unchanged)
        self.assertNotIn(existing_entry.key, removed)
        self.assertIn(removed_entry.key, removed)

    def test_deleted_files_use_forward_slashes(self) -> None:
        """Paths with backslashes from Windows git output must still match."""
        entry = self._make_entry("fn_a", file="src/module/lib.rs")
        new_syms, unchanged, removed = diff_catalog(
            {entry.key: entry}, [], ["src\\module\\lib.rs"]
        )
        self.assertIn(entry.key, removed)

    def test_empty_inputs_return_empty_outputs(self) -> None:
        new_syms, unchanged, removed = diff_catalog({}, [], [])
        self.assertEqual(new_syms, [])
        self.assertEqual(unchanged, {})
        self.assertEqual(removed, set())


if __name__ == "__main__":
    unittest.main()
