"""Unit tests for calculate_release_version.py.

Run with: python -m pytest .github/scripts/test_calculate_release_version.py -v
"""
from __future__ import annotations

import pytest

from calculate_release_version import (
    bump_base,
    calculate_version,
    detect_current_level,
    latest_stable_from_tags,
    next_dry_run_counter,
    next_prerelease_counter,
    parse_version,
    to_pep440,
)


# ---------- parse_version ----------


class TestParseVersion:
    def test_stable(self):
        v = parse_version("1.2.3")
        assert (v.major, v.minor, v.patch) == (1, 2, 3)
        assert v.prerelease_label is None
        assert v.prerelease_num is None

    def test_prerelease(self):
        v = parse_version("0.12.0-next.5")
        assert (v.major, v.minor, v.patch) == (0, 12, 0)
        assert v.prerelease_label == "next"
        assert v.prerelease_num == 5

    @pytest.mark.parametrize("label", ["alpha", "beta", "rc", "next"])
    def test_all_prerelease_labels(self, label):
        v = parse_version(f"1.2.3-{label}.7")
        assert v.prerelease_label == label
        assert v.prerelease_num == 7

    def test_invalid(self):
        with pytest.raises(ValueError):
            parse_version("not-a-version")


# ---------- bump_base ----------


class TestBumpBase:
    @pytest.mark.parametrize(
        "base,bump,expected",
        [
            ("0.11.7", "patch", "0.11.8"),
            ("0.11.7", "minor", "0.12.0"),
            ("0.11.7", "major", "1.0.0"),
            ("1.2.3", "minor", "1.3.0"),
            ("0.0.0", "patch", "0.0.1"),
        ],
    )
    def test_bump(self, base, bump, expected):
        assert bump_base(base, bump) == expected

    def test_unknown_bump(self):
        with pytest.raises(ValueError):
            bump_base("1.0.0", "bogus")


# ---------- detect_current_level ----------


class TestDetectCurrentLevel:
    @pytest.mark.parametrize(
        "current,stable,expected",
        [
            ("0.11.7", "0.11.6", "patch"),
            ("0.12.0", "0.11.6", "minor"),
            ("1.0.0", "0.11.6", "major"),
            ("0.11.6", "0.11.6", None),  # equal — no bump implied
            ("0.10.0", "0.11.6", None),  # current older
        ],
    )
    def test_levels(self, current, stable, expected):
        assert detect_current_level(current, stable) == expected

    def test_no_stable(self):
        assert detect_current_level("0.1.0", None) is None


# ---------- next_prerelease_counter ----------


class TestPrereleaseCounter:
    def test_empty(self):
        assert next_prerelease_counter("0.12.0", "next", [], "iii") == 1

    def test_existing(self):
        tags = ["iii/v0.12.0-next.1", "iii/v0.12.0-next.2", "iii/v0.12.0-next.3"]
        assert next_prerelease_counter("0.12.0", "next", tags, "iii") == 4

    def test_different_base_ignored(self):
        tags = ["iii/v0.11.0-next.1", "iii/v0.13.0-next.5"]
        assert next_prerelease_counter("0.12.0", "next", tags, "iii") == 1

    def test_different_label_ignored(self):
        tags = ["iii/v0.12.0-rc.1", "iii/v0.12.0-alpha.3"]
        assert next_prerelease_counter("0.12.0", "next", tags, "iii") == 1

    def test_different_prefix_ignored(self):
        tags = ["other/v0.12.0-next.5"]
        assert next_prerelease_counter("0.12.0", "next", tags, "iii") == 1

    def test_picks_max_not_count(self):
        tags = ["iii/v0.12.0-next.1", "iii/v0.12.0-next.7"]
        assert next_prerelease_counter("0.12.0", "next", tags, "iii") == 8


# ---------- next_dry_run_counter ----------


class TestDryRunCounter:
    def test_empty(self):
        assert next_dry_run_counter("0.12.0", [], "iii") == 1

    def test_existing(self):
        tags = ["iii/v0.12.0-dry-run.1", "iii/v0.12.0-dry-run.4"]
        assert next_dry_run_counter("0.12.0", tags, "iii") == 5


# ---------- latest_stable_from_tags ----------


class TestLatestStable:
    def test_picks_highest(self):
        tags = [
            "iii/v0.11.5",
            "iii/v0.11.6",
            "iii/v0.11.6-next.3",  # prerelease ignored
            "iii/v0.10.0",
        ]
        assert latest_stable_from_tags(tags, "iii") == "0.11.6"

    def test_no_stable(self):
        tags = ["iii/v0.12.0-next.1", "iii/v0.12.0-next.2"]
        assert latest_stable_from_tags(tags, "iii") is None

    def test_empty(self):
        assert latest_stable_from_tags([], "iii") is None

    def test_different_prefix_ignored(self):
        tags = ["other/v9.9.9", "iii/v0.1.0"]
        assert latest_stable_from_tags(tags, "iii") == "0.1.0"


# ---------- calculate_version ----------


class TestCalculateVersion:
    """All the version-bump scenarios.

    The naming convention is: starting_state__bump_prerelease__expected.
    """

    # -- Stable starting point --

    def test_stable_patch_stable(self):
        assert calculate_version("0.11.6", "patch", "none", "0.11.6", [], "iii") == "0.11.7"

    def test_stable_minor_stable(self):
        assert calculate_version("0.11.6", "minor", "none", "0.11.6", [], "iii") == "0.12.0"

    def test_stable_major_stable(self):
        assert calculate_version("0.11.6", "major", "none", "0.11.6", [], "iii") == "1.0.0"

    def test_stable_patch_next(self):
        assert calculate_version("0.11.6", "patch", "next", "0.11.6", [], "iii") == "0.11.7-next.1"

    def test_stable_minor_next(self):
        assert calculate_version("0.11.6", "minor", "next", "0.11.6", [], "iii") == "0.12.0-next.1"

    def test_stable_major_rc(self):
        assert calculate_version("0.11.6", "major", "rc", "0.11.6", [], "iii") == "1.0.0-rc.1"

    # -- Patch prerelease train --

    def test_patch_prerelease_patch_next_continues_train(self):
        tags = ["iii/v0.11.7-next.1", "iii/v0.11.7-next.2"]
        assert (
            calculate_version("0.11.7-next.2", "patch", "next", "0.11.6", tags, "iii")
            == "0.11.7-next.3"
        )

    def test_patch_prerelease_minor_next_escalates(self):
        """The bug from the original report: minor must escalate, not continue counter."""
        assert (
            calculate_version("0.11.7-next.2", "minor", "next", "0.11.6", [], "iii")
            == "0.12.0-next.1"
        )

    def test_patch_prerelease_major_next_escalates(self):
        assert (
            calculate_version("0.11.7-next.2", "major", "next", "0.11.6", [], "iii")
            == "1.0.0-next.1"
        )

    # -- Minor prerelease train (the second bug the user reported) --

    def test_minor_prerelease_minor_next_continues_train(self):
        """After escalating to 0.12.0-next.1, subsequent minor+next must iterate, not bump again."""
        tags = ["iii/v0.12.0-next.1"]
        assert (
            calculate_version("0.12.0-next.1", "minor", "next", "0.11.6", tags, "iii")
            == "0.12.0-next.2"
        )

    def test_minor_prerelease_patch_next_continues_train(self):
        """Patch is below minor — keep iterating the minor train."""
        tags = ["iii/v0.12.0-next.1"]
        assert (
            calculate_version("0.12.0-next.1", "patch", "next", "0.11.6", tags, "iii")
            == "0.12.0-next.2"
        )

    def test_minor_prerelease_major_next_escalates(self):
        assert (
            calculate_version("0.12.0-next.1", "major", "next", "0.11.6", [], "iii")
            == "1.0.0-next.1"
        )

    # -- Major prerelease train --

    def test_major_prerelease_major_next_continues_train(self):
        tags = ["iii/v1.0.0-next.1", "iii/v1.0.0-next.2"]
        assert (
            calculate_version("1.0.0-next.2", "major", "next", "0.11.6", tags, "iii")
            == "1.0.0-next.3"
        )

    def test_major_prerelease_minor_next_continues_train(self):
        tags = ["iii/v1.0.0-next.1"]
        assert (
            calculate_version("1.0.0-next.1", "minor", "next", "0.11.6", tags, "iii")
            == "1.0.0-next.2"
        )

    # -- Promotion to stable --

    def test_promote_prerelease_to_stable_drops_suffix(self):
        assert calculate_version("0.12.0-next.5", "patch", "none", "0.11.6", [], "iii") == "0.12.0"

    def test_promote_with_minor_bump_still_uses_base(self):
        """Once a prerelease train decided the base, promotion uses it as-is."""
        assert calculate_version("0.12.0-next.5", "minor", "none", "0.11.6", [], "iii") == "0.12.0"

    # -- Channel switching --

    def test_next_to_rc_starts_fresh_counter(self):
        tags = ["iii/v0.12.0-next.3"]
        assert (
            calculate_version("0.12.0-next.3", "minor", "rc", "0.11.6", tags, "iii")
            == "0.13.0-rc.1"
        )

    # -- No prior stable tag (bootstrap) --

    def test_no_stable_falls_back_to_continue_train(self):
        tags = ["iii/v0.1.0-next.1"]
        assert (
            calculate_version("0.1.0-next.1", "minor", "next", None, tags, "iii")
            == "0.1.0-next.2"
        )

    def test_no_stable_stable_starts_from_current(self):
        assert calculate_version("0.0.1", "minor", "next", None, [], "iii") == "0.1.0-next.1"

    # -- Counter resumes from existing max --

    def test_counter_uses_max_existing(self):
        tags = [
            "iii/v0.12.0-next.1",
            "iii/v0.12.0-next.7",
            "iii/v0.12.0-next.3",
        ]
        assert (
            calculate_version("0.12.0-next.7", "minor", "next", "0.11.6", tags, "iii")
            == "0.12.0-next.8"
        )

    # -- Invalid inputs --

    def test_unknown_bump(self):
        with pytest.raises(ValueError):
            calculate_version("0.1.0", "bogus", "none", "0.1.0", [], "iii")

    def test_unknown_prerelease(self):
        with pytest.raises(ValueError):
            calculate_version("0.1.0", "patch", "weird", "0.1.0", [], "iii")


# ---------- to_pep440 ----------


class TestToPep440:
    @pytest.mark.parametrize(
        "version,expected",
        [
            ("0.12.0", "0.12.0"),
            ("0.12.0-rc.1", "0.12.0rc1"),
            ("0.12.0-alpha.3", "0.12.0a3"),
            ("0.12.0-beta.2", "0.12.0b2"),
            ("0.12.0-next.5", "0.12.0.dev5"),
        ],
    )
    def test_conversion(self, version, expected):
        assert to_pep440(version) == expected

    def test_unknown_label_passes_through(self):
        # Dry-run tags aren't valid PEP 440 prereleases; we just leave them.
        assert to_pep440("0.12.0-dry-run.1") == "0.12.0-dry-run.1"
