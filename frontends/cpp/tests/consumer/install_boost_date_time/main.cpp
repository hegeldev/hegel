#include <hegel/extras/boost_date_time.h>
#include <hegel/hegel.h>

#include <iostream>
#include <stdexcept>
#include <string>

namespace gs = hegel::generators;
namespace temporal = hegel::generators::temporal;
namespace lt = boost::local_time;

int main() {
    lt::time_zone_ptr eastern(
        new lt::posix_time_zone("EST-05EDT,M3.2.0/2,M11.1.0/2"));
    temporal::ZoneSource source;
    source.add_zone("Boost/Eastern",
                    hegel::extras::boost_date_time::zone_data(eastern));
    auto when = temporal::zoned_datetimes({.source = source});
    hegel::test([&](hegel::TestCase& tc) {
        temporal::ZonedDateTime drawn = tc.draw(when);
        if (drawn.zone.name != "Boost/Eastern") {
            throw std::runtime_error("drew the zone " + drawn.zone.name);
        }
        int32_t offset = drawn.offset.seconds;
        if (offset != -5 * 3600 && offset != -4 * 3600) {
            throw std::runtime_error("drew the offset " +
                                     std::to_string(offset));
        }
    });
    std::cout << "consumer: all tests passed" << std::endl;
    return 0;
}
