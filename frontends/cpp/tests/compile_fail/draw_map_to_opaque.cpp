#include <hegel/hegel.h>

namespace gs = hegel::generators;

class Sealed {
  public:
    explicit Sealed(int v) : v_(v) {}

  private:
    int v_;
};

void body(hegel::TestCase& tc) {
    (void)tc.draw(gs::integers<int>().map([](int v) { return Sealed(v); }));
}
