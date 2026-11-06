#include <hegel/hegel.h>

namespace gs = hegel::generators;

void body(hegel::TestCase& tc) {
    gs::AnyGenerator<int> silent = gs::integers<int>();
    (void)tc.draw(silent);
}
