# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

# Dropped from the Hypothesis original: test_url_fragments_contain_legal_chars
# (Hypothesis's private _url_fragments_strategy); the max_element_length
# cases of test_invalid_domain_arguments and test_valid_domains_arguments
# (domains(max_element_length=) argument; hegel's domains() has only
# max_length).

import re
import string

import pytest

import hegel
from hegel import InvalidArgument
from hegel import generators as gs
from tests.hypothesis_port.common.debug import check_can_generate_examples, find_any


@hegel.test
def test_is_URL(tc):
    url = tc.draw(gs.urls())
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
def test_invalid_domain_arguments(max_length):
    with pytest.raises(InvalidArgument):
        check_can_generate_examples(gs.domains(max_length=max_length))


@pytest.mark.parametrize("max_length", [4, 8, 255])
def test_valid_domains_arguments(max_length):
    check_can_generate_examples(gs.domains(max_length=max_length))


@pytest.mark.parametrize("strategy", [gs.domains(), gs.urls()])
def test_find_any_non_empty(strategy):
    find_any(strategy, lambda s: len(s) > 0)
