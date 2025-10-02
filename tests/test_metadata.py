from importlib import metadata

import pytest

import syncx


def test_version_matches_package_metadata() -> None:
    assert isinstance(syncx.__version__, str)
    try:
        expected = metadata.version("syncx")
    except metadata.PackageNotFoundError:
        pytest.skip("syncx package metadata is unavailable")
    assert syncx.__version__ == expected
