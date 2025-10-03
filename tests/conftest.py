import pytest


# Apply a default timeout to every test to detect deadlocks quickly.
TIMEOUT_MARK = pytest.mark.timeout(timeout=5, method="thread")


def pytest_collection_modifyitems(config, items):
    for item in items:
        item.add_marker(TIMEOUT_MARK)
