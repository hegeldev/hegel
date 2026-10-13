# Dropped from the Hypothesis original: the clear_database/set_database rules and teardown
# (InMemoryExampleDatabase); complex_numbers(), decimals() and fractions() from the base
# strategies (no hegel generator); the __main__ runner.
# This file is part of Hypothesis, which may be found at
# https://github.com/HypothesisWorks/hypothesis/
#
# Copyright the Hypothesis Authors.
# Individual contributors are listed in AUTHORS.rst and the git log.
#
# This Source Code Form is subject to the terms of the Mozilla Public License,
# v. 2.0. If a copy of the MPL was not distributed with this file, You can
# obtain one at https://mozilla.org/MPL/2.0/.

import hashlib
import math
import struct
from random import Random

import hegel
from hegel import generators as gs


def clamp(lower, value, upper):
    return max(lower, min(value, upper))


def float_to_int(value):
    return struct.unpack("!Q", struct.pack("!d", value))[0]


def int_to_float(value):
    return struct.unpack("!d", struct.pack("!Q", value))[0]


def is_negative(value):
    return math.copysign(1.0, value) < 0


class HypothesisSpec(hegel.StateMachine):
    steps = 50

    def __init__(self, tc):
        self.strategies = hegel.Pool(tc)
        self.strategy_tuples = hegel.Pool(tc)
        self.objects = hegel.Pool(tc)
        self.varied_floats = hegel.Pool(tc)

    @hegel.rule()
    def strategy(self, tc):
        spec = tc.draw(
            gs.sampled_from(
                (
                    gs.integers(),
                    gs.booleans(),
                    gs.floats(),
                    gs.text(),
                    gs.binary(),
                    gs.none(),
                    gs.tuples(),
                )
            )
        )
        self.strategies.add(spec)

    @hegel.rule()
    def sampled_from_strategy(self, tc):
        values = tc.draw(gs.lists(gs.integers() | gs.text(), min_size=1))
        self.strategies.add(gs.sampled_from(values))

    @hegel.rule()
    def strategy_for_tupes(self, tc):
        spec = tc.draw(self.strategy_tuples.values_reusable())
        self.strategies.add(gs.tuples(*spec))

    @hegel.rule()
    def filtered_strategy(self, tc):
        source = tc.draw(self.strategies.values_reusable())
        level = tc.draw(gs.integers(1, 10))
        mixer = tc.draw(gs.text())

        def is_good(x):
            seed = hashlib.sha384((mixer + repr(x)).encode()).digest()
            return bool(Random(seed).randint(0, level))

        self.strategies.add(source.filter(is_good))

    @hegel.rule()
    def list_strategy(self, tc):
        elements = tc.draw(self.strategies.values_reusable())
        self.strategies.add(gs.lists(elements))

    @hegel.rule()
    def or_strategy(self, tc):
        left = tc.draw(self.strategies.values_reusable())
        right = tc.draw(self.strategies.values_reusable())
        self.strategies.add(left | right)

    @hegel.rule()
    def float(self, tc):
        self.varied_floats.add(tc.draw(gs.floats()))

    @hegel.rule()
    def adjust_float(self, tc):
        source = tc.draw(self.varied_floats.values_reusable())
        offset = tc.draw(gs.integers(-100, 100))
        self.varied_floats.add(int_to_float(clamp(0, float_to_int(source) + offset, 2**64 - 1)))

    @hegel.rule()
    def float_range(self, tc):
        left = tc.draw(self.varied_floats.values_reusable())
        right = tc.draw(self.varied_floats.values_reusable())
        tc.assume(math.isfinite(left) and math.isfinite(right))
        left, right = sorted((left, right))
        assert left <= right
        # exclude deprecated case where left = 0.0 and right = -0.0
        tc.assume(left or right or not (is_negative(right) and not is_negative(left)))
        self.strategies.add(gs.floats(left, right))

    @hegel.rule()
    def flatmapped_strategy(self, tc):
        source = tc.draw(self.strategies.values_reusable())
        result1 = tc.draw(self.strategies.values_reusable())
        result2 = tc.draw(self.strategies.values_reusable())
        mixer = tc.draw(gs.text())
        p = tc.draw(gs.floats(0, 1))
        tc.assume(result1 is not result2)

        def do_map(value):
            rep = repr(value)
            random = Random(hashlib.sha384((mixer + rep).encode()).digest())
            if random.random() <= p:
                return result1
            else:
                return result2

        self.strategies.add(source.flat_map(do_map))

    @hegel.rule()
    def just_strategy(self, tc):
        value = tc.draw(self.objects.values_reusable())
        self.strategies.add(gs.just(value))

    @hegel.rule()
    def single_tuple(self, tc):
        source = tc.draw(self.strategies.values_reusable())
        self.strategy_tuples.add((source,))

    @hegel.rule()
    def cat_tuples(self, tc):
        left = tc.draw(self.strategy_tuples.values_reusable())
        right = tc.draw(self.strategy_tuples.values_reusable())
        self.strategy_tuples.add(left + right)

    @hegel.rule()
    def get_example(self, tc):
        strat = tc.draw(self.strategies.values_reusable())
        self.objects.add(tc.draw(strat))

    @hegel.rule()
    def integer_range(self, tc):
        left = tc.draw(gs.integers())
        right = tc.draw(gs.integers())
        left, right = sorted((left, right))
        self.strategies.add(gs.integers(left, right))

    @hegel.rule()
    def repr_is_good(self, tc):
        strat = tc.draw(self.strategies.values_reusable())
        assert " at 0x" not in repr(strat)


test_Hypothesis = hegel.state_machine_test(HypothesisSpec, test_cases=200)
