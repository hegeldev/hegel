import os
import sys

import pytest

from hegel.hypothesis import Phase, Verbosity, settings
from hegel.hypothesis.database import InMemoryExampleDatabase

collect_ignore_glob = []
for minor in range(11, 20):
    if sys.version_info < (3, minor):
        collect_ignore_glob.append(f"cover/*py3{minor}*.py")

settings.register_profile(
    "default",
    settings.get_profile("default"),
    database=InMemoryExampleDatabase(),
    phases=list(Phase),
    deadline=None,
)
settings.register_profile("speedy", max_examples=5)
settings.register_profile("debug", verbosity=Verbosity.debug)
settings.load_profile(os.getenv("HYPOTHESIS_PROFILE", "default"))


def pytest_configure(config):
    config.addinivalue_line("markers", "slow: slow tests")


@pytest.fixture(scope="function", params=["warns", "raises"])
def warns_or_raises(request):
    from tests.hypothesis_port.common.utils import raises_warning

    if request.param == "raises":
        return raises_warning
    return pytest.warns
