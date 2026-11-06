RELEASE_TYPE: patch

This patch follows the bundled engine's date and time draws, which now take weekday and month masks (`hegel_generate_date`) and a precision (`hegel_generate_time`, `hegel_generate_datetime`). `Dates()` and `Datetimes()` draw exactly what they did: every weekday and month, to the nanosecond.
