# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import hegel
from hegel import generators as gs


def is_hegel_test(f):
    return getattr(f, "is_hegel_test", False)


def test_functions_default_to_not_tests():
    def foo():
        pass

    assert not is_hegel_test(foo)


def test_methods_default_to_not_tests():
    class Foo:
        def foo(self):
            pass

    assert not is_hegel_test(Foo().foo)


def test_detection_of_functions():
    @hegel.test
    def test(tc):
        tc.draw(gs.integers())

    assert is_hegel_test(test)


def test_detection_of_methods():
    class Foo:
        @hegel.test
        def test(self, tc):
            tc.draw(gs.integers())

    assert is_hegel_test(Foo().test)


def test_detection_of_stateful_tests():
    class Stuff(hegel.StateMachine):
        @hegel.rule()
        def a_rule(self, tc):
            tc.draw(gs.integers())

    assert is_hegel_test(hegel.state_machine_test(Stuff))
