#pragma once

#include <string>

#include "hegel/core.h"

namespace hegel::generators {

    // =============================================================================
    // Parameter structs
    // =============================================================================

    /**
     * @brief Parameters for domains() generator.
     */
    struct DomainsParams {
        size_t max_length = 255; ///< Maximum domain name length
    };

    /**
     * @brief Parameters for ip_addresses() generator.
     */
    struct IpAddressesParams {
        std::optional<int> v; ///< IP version: 4, 6, or nullopt for both
    };

    /**
     * @brief Parameters for uuids() generator.
     */
    struct UuidsParams {
        /// RFC 4122 version to force (1-5), or nullopt for any version.
        std::optional<int> version;
    };

    /// @name Strings
    /// @{

    /**
     * @brief Generate valid email addresses.
     *
     * Generates addresses in the format specified by RFC 5322 Section 3.4.1
     * (i.e. `local-part@domain`).
     * Values shrink towards shorter local-parts and host domains.
     *
     * @return Generator producing email-address strings.
     */
    Generator<std::string> emails();

    /**
     * @brief Generate valid domain names.
     *
     * Generates RFC 1035-compliant fully qualified domain names.
     *
     * @param params Length constraints. `max_length` (default 255) must
     *   be in the range [4, 255];
     * @return Generator producing domain-name strings.
     */
    Generator<std::string> domains(DomainsParams params = {});

    /**
     * @brief Generate valid URLs.
     *
     * Generates RFC 3986-compliant URLs with either the `http` or `https`
     * scheme.
     *
     * @return Generator producing http/https URL strings.
     */
    Generator<std::string> urls();

    /// @}

    /// @name Misc
    /// @{

    /**
     * @brief Generate IP addresses.
     *
     * Generates IP addresses serialized to text:
     * - IPv4: dotted-quad form (e.g. `192.0.2.5`).
     * - IPv6: colon-hex form (e.g. `2001:db8::1`).
     *
     * Any address in the selected version's space may be produced.
     *
     *
     * @param params Version constraint: `v = 4` for IPv4 only, `v = 6`
     *   for IPv6 only, or `std::nullopt` (default) for a mix of both.
     * @return Generator producing IP-address strings.
     */
    Generator<std::string> ip_addresses(IpAddressesParams params = {});

    /**
     * @brief Generate UUIDs in canonical hyphenated form.
     *
     * Produces lowercase 8-4-4-4-12 hex strings (e.g.
     * `f47ac10b-58cc-4372-a567-0e02b2c3d479`). By default any version is
     * generated; set `version` to restrict to a specific RFC 4122 version.
     *
     * @param params Version constraint: `version = 1..5` forces that RFC 4122
     *   version, or `std::nullopt` (default) for any version.
     * @return Generator producing canonical UUID strings.
     */
    Generator<std::string> uuids(UuidsParams params = {});

    /// @}

    /// @name Datetime (deprecated)
    /// @{

    /**
     * @brief Generate ISO 8601 date strings (`YYYY-MM-DD`) from 0001-01-01
     * to 9999-12-31.
     *
     * @deprecated Use temporal::dates(), which draws a temporal::Date.
     * @return Generator producing ISO 8601 date strings.
     */
    [[deprecated("use hegel::generators::temporal::dates()")]]
    Generator<std::string> dates();

    /**
     * @brief Generate ISO 8601 time strings (`HH:MM:SS[.ffffff]`, the
     * fraction present iff the microsecond is nonzero).
     *
     * @deprecated Use temporal::times(), which draws a temporal::Time.
     * @return Generator producing ISO 8601 time strings.
     */
    [[deprecated("use hegel::generators::temporal::times()")]]
    Generator<std::string> times();

    /**
     * @brief Generate naive ISO 8601 datetime strings
     * (`YYYY-MM-DDTHH:MM:SS[.ffffff]`) from 0001-01-01 to 9999-12-31.
     *
     * @deprecated Use temporal::datetimes(), which draws a
     * temporal::DateTime.
     * @return Generator producing ISO 8601 datetime strings.
     */
    [[deprecated("use hegel::generators::temporal::datetimes()")]]
    Generator<std::string> datetimes();

    /// @}

} // namespace hegel::generators
