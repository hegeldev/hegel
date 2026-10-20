import time

from hegel.generators import Generator, binary, integers


class _Slow(Generator):
    def do_draw(self, tc):
        time.sleep(1.01)
        return tc.draw_silent(binary(min_size=2, max_size=2))


SLOW = _Slow()


class HardToShrink(Generator):
    def __init__(self):
        super().__init__()
        self.__last = None
        self.accepted = set()

    def do_draw(self, tc):
        x = bytes(tc.draw_silent(integers(0, 255)) for _ in range(100))
        if x in self.accepted:
            return True
        ls = self.__last
        if ls is None:
            if all(x):
                self.__last = x
                self.accepted.add(x)
                return True
            else:
                return False
        diffs = [i for i in range(len(x)) if x[i] != ls[i]]
        if len(diffs) == 1:
            i = diffs[0]
            if x[i] + 1 == ls[i]:
                self.__last = x
                self.accepted.add(x)
                return True
        return False
