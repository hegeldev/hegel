#pragma once

#include <regex>
#include <string>

#include <ApprovalTests.hpp>

// Scrubbers shared by the snapshot suites. Each hides a part of a failure
// report that comes from the environment, so the snapshots pin the layout
// around a placeholder.

namespace hegel::tests::common {

    /// @brief Hides a report's reproduction blob.
    ///
    /// A blob encodes engine internals and changes with the engine version.
    inline ApprovalTests::Options scrub_blob() {
        return ApprovalTests::Options(
            ApprovalTests::Scrubbers::createRegexScrubber(
                std::regex(R"("[A-Za-z0-9+/=]{8,}")"), R"("[blob]")"));
    }

    /// @brief Hides a report's reproduction blob and the source position in
    /// its header.
    ///
    /// A header that hegel::test() fills in holds the absolute path the
    /// compiler was given and the line of the call, which moves with the file
    /// above it. Use this for a report Hegel named itself.
    inline ApprovalTests::Options scrub_report() {
        return ApprovalTests::Options([](const std::string& text) {
            std::string blobless = std::regex_replace(
                text, std::regex(R"("[A-Za-z0-9+/=]{8,}")"), R"("[blob]")");
            return std::regex_replace(
                blobless, std::regex(R"((--- Failure: [^(\n]+\()[^\n]*)"),
                "$1[location]) ---");
        });
    }

    inline ApprovalTests::Options scrub_concurrent_report() {
        return ApprovalTests::Options([](const std::string& text) {
            std::string blobless = std::regex_replace(
                text, std::regex(R"("[A-Za-z0-9+/=]{8,}")"), R"("[blob]")");
            std::string locationless = std::regex_replace(
                blobless, std::regex(R"((--- Failure: [^(\n]+\()[^\n]*)"),
                "$1[location]) ---");
            return std::regex_replace(
                locationless,
                std::regex(R"(\[worker ([0-9]+) \+[0-9]+\.[0-9]{3}ms\])"),
                "[worker $1 +time]");
        });
    }

} // namespace hegel::tests::common
