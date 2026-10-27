> [!IMPORTANT]
> We're excited you're checking out Hegel! Hegel is in beta, and we'd love for you to try it and [report any feedback](https://github.com/hegeldev/hegel/issues/new).
>
> As part of our beta, we may make breaking changes if it makes Hegel a better property-based testing library. If that instability bothers you, please check back in a few months for a stable release!
>
> See https://hegel.dev/compatibility for more details.

# Hegel for C++

* [Documentation](https://hegel.dev/cpp)
* [Website](https://hegel.dev)

Hegel is a property-based testing library for C++. Hegel is based on [Hypothesis](https://github.com/hypothesisworks/hypothesis), using [libhegel](https://github.com/hegeldev/hegel/tree/main/libhegel), Hegel's native engine.

## Installation

Hegel requires C++20.

To install with CMake, add this to your `CMakeLists.txt` (CMake 3.14+ required):

```cmake
include(FetchContent)
FetchContent_Declare(
    hegel
    GIT_REPOSITORY https://github.com/hegeldev/hegel.git
    GIT_TAG frontends/cpp/0.13.2
    SOURCE_SUBDIR frontends/cpp
)
FetchContent_MakeAvailable(hegel)

target_link_libraries(your_target PRIVATE hegel)
```

The library lives in the [hegeldev/hegel](https://github.com/hegeldev/hegel)
repository next to [libhegel](https://github.com/hegeldev/hegel/tree/main/libhegel),
Hegel's native engine, which the build compiles with `cargo` (a Rust
toolchain from [rustup](https://rustup.rs)) and links. To link a libhegel
built elsewhere instead, so that no Rust toolchain is needed, pass
`-DHEGEL_LIBHEGEL_LIBRARY=/path/to/libhegel_c.<ext>`.
See https://hegel.dev/reference/installation for details.

## Quickstart

Here's a quick example of how to write a Hegel test:

```cpp
#include <hegel/hegel.h>
#include <algorithm>
#include <stdexcept>
#include <vector>

namespace gs = hegel::generators;

std::vector<int> my_sort(std::vector<int> ls) {
    std::sort(ls.begin(), ls.end());
    ls.erase(std::unique(ls.begin(), ls.end()), ls.end());
    return ls;
}

HEGEL_TEST(sort_agrees_with_std_sort)(hegel::TestCase& tc) {
    auto vec1 = tc.draw("vec1", gs::vectors(gs::integers<int>()));
    auto vec2 = my_sort(vec1);
    std::sort(vec1.begin(), vec1.end());
    if (vec1 != vec2) {
        throw std::runtime_error("sort mismatch");
    }
}

int main() {
    sort_agrees_with_std_sort();
    return 0;
}
```

`HEGEL_TEST` defines the test as a plain function you can invoke from `main()`
or from any test framework, and names the test in Hegel's example database so
failures found in one run are replayed first in the next. (You can also call
`hegel::test(callback, settings)` directly.)

This test will fail! Hegel will produce a minimal failing test case for us:

```
auto vec1 = std::vector<int>{0, 0};

To reproduce this failure, add this line above HEGEL_TEST(sort_agrees_with_std_sort):
    HEGEL_REPRODUCE_FAILURE(sort_agrees_with_std_sort, "AXicY2VgYGBkZOBiZEBhMAAAAd8AIQ==")
```

The report shows the minimal example and how to replay it, and the test's own
exception (`sort mismatch`) propagates out of the test.

Each draw is replayed and printed as a C++ declaration. `tc.draw("vec1", gen)`
names the draw with `vec1` as above. An unnamed `tc.draw(gen)` prints
(`auto draw_1 = ...;`) instead. A value too wide for one line is broken
across lines, one element per line.

The last line replays this exact failure. Put it above the test to rerun the
counterexample instead of generating new cases:

```cpp
HEGEL_REPRODUCE_FAILURE(sort_agrees_with_std_sort, "AXicY2VgYGBkZOBiZEBhMAAAAd8AIQ==")
HEGEL_TEST(sort_agrees_with_std_sort)(hegel::TestCase& tc) {
    // ...
}
```

### How values print

The generator prints the value as it draws it. A container prints each
element as the element's generator prints it, so printing that you choose for
one generator shows inside every value built from it. `tc.draw` needs a
printable generator, `hegel::generators::Generator<T>`. Every built-in
generator of a type with a C++ representation is one. Arithmetic types, enums,
`std::string` and `std::monostate` have one, and so do the standard pairs,
tuples, variants, optionals, vectors, sets, arrays and maps of such types. A
type of your own has one when it has a `void print_to(hegel::Printer&) const`
member or an `operator<<`, or (C++20) is a reflectable aggregate.

A generator that cannot print is an `AnyGenerator<T>`. For example, `map` to
a type with no C++ representation gives one. Make it printable with one of
these:

```cpp
namespace gs = hegel::generators;

// Print each value with your own function.
auto ids = gs::integers<int>({.min_value = 0})
               .print_with([](int id, hegel::Printer& p) {
                   p.text("UserId(" + std::to_string(id) + ")");
               });

// Print a mapped value as a call on the value it was made from:
// `auto key = KeyData::from_ffi(3);`
auto keys = gs::integers<int>()
                .map([](int raw) { return KeyData::from_ffi(raw); })
                .print_as_call("KeyData::from_ffi");
```

`print_as_repr()` prints by the C++ representation, and `tc.draw_silent(gen)`
draws from any generator and prints nothing. A generator of your own
implements `hegel::generators::IPrintableGenerator<T>` and writes its value to
a `hegel::Printer` in `do_draw_and_print`; see the API reference.

A reflectable aggregate is representable even when one of its fields is not.
That field prints as an `<unprintable ...>` placeholder. Give the field's type
a representation, or draw the aggregate with `print_with`.
