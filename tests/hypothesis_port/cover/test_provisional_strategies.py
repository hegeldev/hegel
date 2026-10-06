# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import re
import string

import pytest

from hegel.hypothesis import given, settings
from hegel.hypothesis.errors import InvalidArgument
from hegel.hypothesis.provisional import domains, urls

from tests.hypothesis_port.common.debug import check_can_generate_examples, find_any


@given(urls())
def test_is_URL(url):
    allowed_chars = set(string.ascii_letters + string.digits + "$-_.+!*'(),~%/")
    url_schemeless = url.split("://", 1)[1]
    components = url_schemeless.split("#", 1)

    domain_path = components[0]
    path = domain_path.split("/", 1)[1] if "/" in domain_path else ""
    assert all(c in allowed_chars for c in path)
    assert all(re.match("^[0-9A-Fa-f]{2}", after_perc) for after_perc in path.split("%")[1:])

    fragment = components[1] if "#" in url_schemeless else ""
    fragment_allowed_chars = allowed_chars | {"?"}
    assert all(c in fragment_allowed_chars for c in fragment)
    assert all(re.match("^[0-9A-Fa-f]{2}", after_perc) for after_perc in fragment.split("%")[1:])


@pytest.mark.parametrize("max_length", [-1, 0, 3, 4.0, 256])
@pytest.mark.parametrize("max_element_length", [-1, 0, 4.0, 64, 128])
def test_invalid_domain_arguments(max_length, max_element_length):
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(
            domains(max_length=max_length, max_element_length=max_element_length)
        )


@pytest.mark.skipif(
    settings.get_current_profile_name() == "crosshair",
    reason="takes ~300s each; the `sampled_from(get_top_level_domains())` decision is realized via iterative comparisons https://github.com/pschanely/CrossHair/issues/332",
)
@pytest.mark.parametrize("max_length", [None, 4, 8, 255])
@pytest.mark.parametrize("max_element_length", [None, 1, 2, 4, 8, 63])
def test_valid_domains_arguments(max_length, max_element_length):
    check_can_generate_examples(
        domains(max_length=max_length, max_element_length=max_element_length)
    )


@pytest.mark.parametrize("strategy", [domains(), urls()])
def test_find_any_non_empty(strategy):
    find_any(strategy, lambda s: len(s) > 0)


# Dropped: test_url_fragments_contain_legal_chars tests Hypothesis's private _url_fragments_strategy.
