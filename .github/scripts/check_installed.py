"""Smoke-test an installed hegeltest distribution (run from outside the checkout).

Checks that the package imports from site-packages rather than the source
tree, that the native extension loads, that the version matches Cargo.toml,
and that the py.typed marker shipped.
"""

import importlib.metadata
import importlib.resources
import re
import sys
from pathlib import Path

import hegel
from hegel import generators as gs

cargo_toml = Path(__file__).resolve().parents[2] / "Cargo.toml"
expected = re.search(r'^version = "([^"]+)"', cargo_toml.read_text(), re.MULTILINE)
assert expected is not None

assert "site-packages" in hegel.__file__, hegel.__file__
assert hegel.__version__ == importlib.metadata.version("hegeltest") == expected.group(1), (
    hegel.__version__
)
assert importlib.resources.files("hegel").joinpath("py.typed").is_file()


@hegel.test(test_cases=10)
def test_runs(tc: hegel.TestCase) -> None:
    tc.draw(gs.integers())


test_runs()
print(f"hegel {hegel.__version__} (engine {hegel.engine_version()}) on {sys.version}")
