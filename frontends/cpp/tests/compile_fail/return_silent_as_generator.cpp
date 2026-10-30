#include <hegel/hegel.h>

namespace gs = hegel::generators;

gs::Generator<int> helper() {
    gs::AnyGenerator<int> silent = gs::integers<int>();
    return silent;
}
