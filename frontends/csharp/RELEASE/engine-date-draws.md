RELEASE_TYPE: patch

This patch follows the bundled engine's date and time draws, which now take weekday and month masks (`hegel_generate_date`) and a precision (`hegel_generate_time`, `hegel_generate_datetime`). `Generators.Dates()`, `Generators.Times()` and `Generators.DateTimes()` draw exactly what they did: every weekday and month, to the nanosecond before it is rounded down to a .NET tick.
