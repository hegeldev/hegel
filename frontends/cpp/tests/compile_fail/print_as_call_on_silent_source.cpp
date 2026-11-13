#include <hegel/hegel.h>

namespace gs = hegel::generators;

void body() {
    gs::AnyGenerator<int> silent = gs::integers<int>();
    (void)silent.map([](int v) { return v + 1; }).print_as_call("f");
}
