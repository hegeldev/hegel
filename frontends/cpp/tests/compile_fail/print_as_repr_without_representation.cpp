#include <hegel/hegel.h>

namespace gs = hegel::generators;

class Sealed {
  public:
    explicit Sealed(int v) : v_(v) {}

  private:
    int v_;
};

void body() {
    (void)gs::AnyGenerator<Sealed>(gs::just(Sealed(1))).print_as_repr();
}
