import sys
from collections.abc import Iterator

import pytest

import hegel

collect_ignore_glob = []
for minor in range(11, 20):
    if sys.version_info < (3, minor):
        collect_ignore_glob.append(f"cover/*py3{minor}*.py")


def pytest_configure(config):
    config.addinivalue_line("markers", "slow: slow tests")


@pytest.fixture(autouse=True)
def _port_default_profile(_quiet_default_profile) -> Iterator[None]:
    """Hypothesis's defaults, as far as Hegel has them: 100 test cases, no
    example database, and no reproduce blobs in the output."""
    hegel.Settings.register_profile(
        "hypothesis-port",
        hegel.Settings.from_profile("development"),
        database="",
        test_cases=100,
        print_blob=False,
        verbosity=hegel.Verbosity.normal,
        seed=None,
        derandomize=False,
    )
    hegel.Settings.set_default_profile("hypothesis-port")
    yield
