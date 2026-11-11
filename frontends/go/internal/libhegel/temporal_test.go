package libhegel

import (
	"errors"
	"runtime"
	"strings"
	"sync/atomic"
	"testing"
	"time"
	"unsafe"
)

func realTestCase(t *testing.T) (*Context, *TestCase) {
	t.Helper()
	ctx := NewContext()
	settings, err := ctx.SettingsNew()
	if err != nil {
		t.Fatal(err)
	}
	for _, err := range []error{settings.Database(ctx, ""), settings.Derandomize(ctx, true)} {
		if err != nil {
			t.Fatal(err)
		}
	}
	run, err := settings.RunStart(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	tc, err := run.NextTestCase(ctx)
	if err != nil || tc == nil {
		t.Fatalf("test case = %v, %v", tc, err)
	}
	// A collected run completes its test cases, so it has to outlive the test.
	t.Cleanup(func() { runtime.KeepAlive(run) })
	return ctx, tc
}

func wantErrorContaining(t *testing.T, err error, want string) {
	t.Helper()
	if err == nil || !strings.Contains(err.Error(), want) {
		t.Fatalf("error = %v, want one containing %q", err, want)
	}
}

func TestTemporalStructLayouts(t *testing.T) {
	t.Parallel()
	var (
		zoned  ZonedDatetime
		info   timezoneInfoT
		ztype  zoneTypeT
		oracle zoneOracleT
	)
	for name, pair := range map[string][2]uintptr{
		"sizeof Timestamp":             {unsafe.Sizeof(Timestamp{}), 16},
		"sizeof Duration":              {unsafe.Sizeof(Duration{}), 16},
		"sizeof Offset":                {unsafe.Sizeof(Offset{}), 8},
		"sizeof ZonedDatetime":         {unsafe.Sizeof(zoned), 56},
		"ZonedDatetime.OffsetSeconds":  {unsafe.Offsetof(zoned.OffsetSeconds), 16},
		"ZonedDatetime.OffsetKnown":    {unsafe.Offsetof(zoned.OffsetKnown), 20},
		"ZonedDatetime.Fold":           {unsafe.Offsetof(zoned.Fold), 21},
		"ZonedDatetime.Kind":           {unsafe.Offsetof(zoned.Kind), 24},
		"ZonedDatetime.OffsetBefore":   {unsafe.Offsetof(zoned.OffsetBefore), 28},
		"ZonedDatetime.OffsetAfter":    {unsafe.Offsetof(zoned.OffsetAfter), 32},
		"ZonedDatetime.Timestamp":      {unsafe.Offsetof(zoned.Timestamp), 40},
		"sizeof timezoneInfoT":         {unsafe.Sizeof(info), 32},
		"timezoneInfoT.primaryName":    {unsafe.Offsetof(info.primaryName), 8},
		"timezoneInfoT.isLink":         {unsafe.Offsetof(info.isLink), 24},
		"timezoneInfoT.rank":           {unsafe.Offsetof(info.rank), 26},
		"timezoneInfoT.tags":           {unsafe.Offsetof(info.tags), 28},
		"sizeof zoneTypeT":             {unsafe.Sizeof(ztype), 24},
		"zoneTypeT.isDST":              {unsafe.Offsetof(ztype.isDST), 4},
		"zoneTypeT.abbreviation":       {unsafe.Offsetof(ztype.abbreviation), 8},
		"zoneTypeT.abbreviationLen":    {unsafe.Offsetof(ztype.abbreviationLen), 16},
		"sizeof zoneOracleT":           {unsafe.Sizeof(oracle), 40},
		"zoneOracleT.nextTransition":   {unsafe.Offsetof(oracle.nextTransition), 8},
		"zoneOracleT.userData":         {unsafe.Offsetof(oracle.userData), 16},
		"zoneOracleT.firstYear":        {unsafe.Offsetof(oracle.firstYear), 24},
		"zoneOracleT.lastYear":         {unsafe.Offsetof(oracle.lastYear), 28},
		"zoneOracleT.after":            {unsafe.Offsetof(oracle.after), 32},
		"sizeof hegel_string_t (name)": {unsafe.Sizeof(stringT{}), 2 * unsafe.Sizeof(uintptr(0))},
	} {
		if pair[0] != pair[1] {
			t.Errorf("%s = %d, want %d", name, pair[0], pair[1])
		}
	}
}

func TestDurationDrawPassesBoundsByValue(t *testing.T) {
	ctx, tc := realTestCase(t)
	for _, want := range []Duration{
		{Seconds: 0, Nanoseconds: 0},
		{Seconds: -5, Nanoseconds: -250},
		{Seconds: 1 << 40, Nanoseconds: 999999999},
	} {
		got, err := tc.GenerateDuration(ctx, want, want, PRECISION_NANOSECONDS)
		if err != nil || got != want {
			t.Fatalf("GenerateDuration(%+v) = %+v, %v", want, got, err)
		}
	}
	got, err := tc.GenerateDuration(ctx, Duration{Seconds: 1}, Duration{Seconds: 7199}, PRECISION_HOURS)
	if err != nil || got != (Duration{Seconds: 3600}) {
		t.Fatalf("hour-precision duration = %+v, %v", got, err)
	}
	_, err = tc.GenerateDuration(ctx, Duration{Seconds: 2}, Duration{Seconds: 1}, PRECISION_NANOSECONDS)
	wantErrorContaining(t, err, "hegel_generate_duration")
}

func TestOffsetDrawAndFormat(t *testing.T) {
	ctx, tc := realTestCase(t)
	got, err := tc.GenerateOffset(ctx, 19800, 19800, OFFSET_GRANULARITY_MINUTES, false)
	if err != nil || got != (Offset{Seconds: 19800, Known: true}) {
		t.Fatalf("GenerateOffset = %+v, %v", got, err)
	}
	got, err = tc.GenerateOffset(ctx, -2670, -2670, OFFSET_GRANULARITY_SECONDS, false)
	if err != nil || got != (Offset{Seconds: -2670, Known: true}) {
		t.Fatalf("GenerateOffset = %+v, %v", got, err)
	}
	_, err = tc.GenerateOffset(ctx, 1, 59, OFFSET_GRANULARITY_HOURS, false)
	wantErrorContaining(t, err, "hegel_generate_offset")

	for offset, want := range map[Offset]string{
		{Seconds: 19800, Known: true}: "+05:30",
		{Seconds: -2670, Known: true}: "-00:44:30",
		{Seconds: 0, Known: true}:     "+00:00",
		{}:                            "-00:00",
	} {
		text, err := ctx.FormatOffset(offset)
		if err != nil || text != want {
			t.Fatalf("FormatOffset(%+v) = %q, %v; want %q", offset, text, err, want)
		}
	}
}

func TestDateOptionsMasks(t *testing.T) {
	ctx, tc := realTestCase(t)
	options, err := ctx.DateOptionsNew()
	if err != nil {
		t.Fatal(err)
	}
	defer options.Free()
	for _, err := range []error{options.SetWeekdays(ctx, 1<<6), options.SetMonths(ctx, 1<<1)} {
		if err != nil {
			t.Fatal(err)
		}
	}
	for range 20 {
		d, err := tc.GenerateDate(ctx, Date{Year: 1990, Month: 1, Day: 1}, Date{Year: 2030, Month: 12, Day: 31}, options)
		if err != nil {
			t.Fatal(err)
		}
		if got := d.ToTime(); got.Weekday() != time.Sunday || got.Month() != time.February {
			t.Fatalf("drew %v, want a Sunday in February", got)
		}
	}
	wantErrorContaining(t, options.SetWeekdays(ctx, 0), "hegel_date_options_set_weekdays")
	wantErrorContaining(t, options.SetMonths(ctx, 1<<12), "hegel_date_options_set_months")
}

// stepInstant is where the test zone's clocks go from UTC to an hour ahead:
// 2001-09-09T01:46:40Z.
const stepInstant = 1_000_000_000

func stepOracle() ZoneOracle {
	return ZoneOracle{
		OffsetAt: func(instant int64) (ZoneType, error) {
			if instant < stepInstant {
				return ZoneType{Offset: 0, Abbreviation: "AAA"}, nil
			}
			return ZoneType{Offset: 3600, IsDST: true, Abbreviation: "BBB"}, nil
		},
		NextTransition: func(after int64) (int64, bool, error) {
			return stepInstant, after < stepInstant, nil
		},
		FirstYear: 1970,
		LastYear:  2030,
		After:     ZONE_AFTER_HOLD,
	}
}

func civil(year int32, month, day, hour uint8) Datetime {
	return Datetime{Date: Date{Year: year, Month: month, Day: day}, Time: Time{Hour: hour}}
}

func drawPinned(t *testing.T, ctx *Context, tc *TestCase, source *ZoneSource, name string, at Datetime) ZonedDatetime {
	t.Helper()
	spec, err := ctx.ZonedSpecNew()
	if err != nil {
		t.Fatal(err)
	}
	defer spec.Free()
	if err := spec.SetCivilBounds(ctx, at, at); err != nil {
		t.Fatal(err)
	}
	value, err := tc.GenerateZonedDatetimeForZone(ctx, source, name, spec)
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func TestZoneSourceOracleProbe(t *testing.T) {
	ctx, tc := realTestCase(t)
	sampled := stepOracle()
	sampled.NextTransition = nil
	for label, oracle := range map[string]ZoneOracle{"walked": stepOracle(), "sampled": sampled} {
		source, err := ctx.ZoneSourceNew()
		if err != nil {
			t.Fatal(err)
		}
		if err := source.AddOracle(ctx, "Test/Step", oracle); err != nil {
			t.Fatalf("%s: AddOracle: %v", label, err)
		}
		before := drawPinned(t, ctx, tc, source, "Test/Step", civil(2001, 9, 9, 1))
		if before.OffsetSeconds != 0 || !before.OffsetKnown || before.Kind != TIMEZONE_KIND_UNIQUE || before.Timestamp.Second != stepInstant-2800 {
			t.Fatalf("%s: before the step = %+v", label, before)
		}
		after := drawPinned(t, ctx, tc, source, "Test/Step", civil(2001, 9, 9, 3))
		if after.OffsetSeconds != 3600 || after.Timestamp.Second != stepInstant+800 || after.Civil != civil(2001, 9, 9, 3) {
			t.Fatalf("%s: after the step = %+v", label, after)
		}
		source.Free()
	}
}

func TestZoneSourceOracleFailures(t *testing.T) {
	ctx := NewContext()
	source, err := ctx.ZoneSourceNew()
	if err != nil {
		t.Fatal(err)
	}
	defer source.Free()

	failing := stepOracle()
	failing.OffsetAt = func(int64) (ZoneType, error) { return ZoneType{}, errors.New("no offset here") }
	wantErrorContaining(t, source.AddOracle(ctx, "Test/Fail", failing), "no offset here")

	failing = stepOracle()
	failing.NextTransition = func(int64) (int64, bool, error) { return 0, false, errors.New("no transition here") }
	wantErrorContaining(t, source.AddOracle(ctx, "Test/Fail", failing), "no transition here")

	failing = stepOracle()
	failing.OffsetAt = nil
	wantErrorContaining(t, source.AddOracle(ctx, "Test/Fail", failing), "offset_at is null")

	info, err := ctx.TimezoneInfoIn(source, "Test/Fail")
	if err != nil || info.Known {
		t.Fatalf("a zone whose probe failed is known: %+v, %v", info, err)
	}
}

func TestZoneSourceLoaderSuppliesDeclaredNames(t *testing.T) {
	ctx, tc := realTestCase(t)
	source, err := ctx.ZoneSourceNew()
	if err != nil {
		t.Fatal(err)
	}
	if err := source.Declare(ctx, []string{"Test/Step", "Test/Broken"}); err != nil {
		t.Fatal(err)
	}
	var asked []string
	loader := func(ctx *Context, source *ZoneSource, name string) error {
		asked = append(asked, name)
		switch name {
		case "Test/Step":
			return source.AddOracle(ctx, name, stepOracle())
		case "Test/Broken":
			return errors.New("the loader cannot read Test/Broken")
		}
		return nil
	}
	if err := source.SetLoader(ctx, loader, nil); err != nil {
		t.Fatal(err)
	}

	info, err := ctx.TimezoneInfoIn(source, "Test/Step")
	if err != nil || !info.Known || info.IsLink || info.PrimaryName != "Test/Step" {
		t.Fatalf("info = %+v, %v", info, err)
	}
	bundled, err := ctx.ZoneSourceBundled()
	if err != nil {
		t.Fatal(err)
	}
	london, err := ctx.TimezoneInfoIn(bundled, "Europe/London")
	if err != nil || !london.Known || london.Rank >= info.Rank {
		t.Fatalf("bundled Europe/London = %+v, %v; custom name rank %d", london, err, info.Rank)
	}
	utc, err := ctx.TimezoneInfoIn(nil, "UTC")
	if err != nil || !utc.Known || utc.Rank != 0 {
		t.Fatalf("default source UTC = %+v, %v", utc, err)
	}
	missing, err := ctx.TimezoneInfoIn(source, "Test/Missing")
	if err != nil || missing.Known {
		t.Fatalf("info for a name the loader lacks = %+v, %v", missing, err)
	}

	tzSpec, err := ctx.TimezoneSpecNew()
	if err != nil {
		t.Fatal(err)
	}
	for _, err := range []error{
		tzSpec.SetSource(ctx, source),
		tzSpec.SetNames(ctx, []string{"Test/Step"}),
		tzSpec.SetLinks(ctx, false),
		tzSpec.SetLegacy(ctx, true),
	} {
		if err != nil {
			t.Fatal(err)
		}
	}
	name, err := tc.GenerateTimezone(ctx, tzSpec)
	if err != nil || name != "Test/Step" {
		t.Fatalf("GenerateTimezone = %q, %v", name, err)
	}

	zonedSpec, err := ctx.ZonedSpecNew()
	if err != nil {
		t.Fatal(err)
	}
	instant := Timestamp{Second: stepInstant + 800, Nanosecond: 5}
	for _, err := range []error{
		zonedSpec.SetInstantBounds(ctx, instant, instant),
		zonedSpec.SetPrecision(ctx, PRECISION_NANOSECONDS),
		zonedSpec.SetImaginary(ctx, IMAGINARY_AVOID),
		zonedSpec.SetAmbiguous(ctx, AMBIGUOUS_LATER),
	} {
		if err != nil {
			t.Fatal(err)
		}
	}
	value, zone, err := tc.GenerateZonedDatetime(ctx, tzSpec, zonedSpec)
	want := civil(2001, 9, 9, 3)
	want.Time.Nanosecond = 5
	if err != nil || zone != "Test/Step" || value.Timestamp != instant || value.Civil != want || value.OffsetSeconds != 3600 {
		t.Fatalf("GenerateZonedDatetime = %+v in %q, %v", value, zone, err)
	}

	_, err = tc.GenerateZonedDatetimeForZone(ctx, source, "Test/Broken", zonedSpec)
	wantErrorContaining(t, err, "the loader cannot read Test/Broken")
	_, err = tc.GenerateZonedDatetimeForZone(ctx, source, "Test/Missing", zonedSpec)
	wantErrorContaining(t, err, "Test/Missing")
	inUTC, err := tc.GenerateZonedDatetimeForZone(ctx, nil, "UTC", zonedSpec)
	if err != nil || inUTC.Timestamp != instant || inUTC.OffsetSeconds != 0 {
		t.Fatalf("draw in the default source's UTC = %+v, %v", inUTC, err)
	}

	if err := tzSpec.SetSource(ctx, nil); err != nil {
		t.Fatal(err)
	}
	_, err = tc.GenerateTimezone(ctx, tzSpec)
	wantErrorContaining(t, err, "Test/Step")
	_, _, err = tc.GenerateZonedDatetime(ctx, tzSpec, zonedSpec)
	wantErrorContaining(t, err, "Test/Step")

	wantErrorContaining(t, tzSpec.SetNames(ctx, nil), "hegel_timezone_spec_set_names")
	wantErrorContaining(t, zonedSpec.SetCivilBounds(ctx, civil(2001, 1, 2, 0), civil(2001, 1, 1, 0)), "hegel_zoned_spec_set_civil_bounds")
	wantErrorContaining(t, zonedSpec.SetInstantBounds(ctx, Timestamp{Second: 2}, Timestamp{Second: 1}), "hegel_zoned_spec_set_instant_bounds")
	wantErrorContaining(t, zonedSpec.SetPrecision(ctx, Precision(99)), "hegel_zoned_spec_set_precision")
	wantErrorContaining(t, zonedSpec.SetImaginary(ctx, Imaginary(99)), "hegel_zoned_spec_set_imaginary")
	wantErrorContaining(t, zonedSpec.SetAmbiguous(ctx, Ambiguous(99)), "hegel_zoned_spec_set_ambiguous")
	wantErrorContaining(t, source.Declare(ctx, []string{""}), "hegel_zone_source_declare")

	if got := strings.Join(asked, ","); got != "Test/Step,Test/Missing,Test/Broken" {
		t.Fatalf("the loader was asked for %q", got)
	}
}

func TestZoneSourceLoaderIsReleasedOnce(t *testing.T) {
	ctx := NewContext()
	source, err := ctx.ZoneSourceNew()
	if err != nil {
		t.Fatal(err)
	}
	noop := func(*Context, *ZoneSource, string) error { return nil }
	var first, second, third atomic.Int32

	if err := source.SetLoader(ctx, noop, func() { first.Add(1) }); err != nil {
		t.Fatal(err)
	}
	if first.Load() != 0 {
		t.Fatal("the loader was released while the source still used it")
	}
	if err := source.SetLoader(ctx, noop, func() { second.Add(1) }); err != nil {
		t.Fatal(err)
	}
	if first.Load() != 1 || second.Load() != 0 {
		t.Fatalf("after replacing the loader: released %d and %d times, want 1 and 0", first.Load(), second.Load())
	}
	if err := source.SetLoader(ctx, nil, nil); err != nil {
		t.Fatal(err)
	}
	if second.Load() != 1 {
		t.Fatalf("after removing the loader: released %d times, want 1", second.Load())
	}

	if err := source.SetLoader(ctx, noop, func() { third.Add(1) }); err != nil {
		t.Fatal(err)
	}
	source.Free()
	if first.Load() != 1 || second.Load() != 1 || third.Load() != 1 {
		t.Fatalf("after freeing the source: released %d, %d and %d times, want 1 each", first.Load(), second.Load(), third.Load())
	}
}

func TestZoneSourceLoaderIsReleasedWhenTheCallFails(t *testing.T) {
	ctx := NewContext()
	bundled, err := ctx.ZoneSourceBundled()
	if err != nil {
		t.Fatal(err)
	}
	var released atomic.Int32
	err = bundled.SetLoader(ctx, func(*Context, *ZoneSource, string) error { return nil }, func() { released.Add(1) })
	wantErrorContaining(t, err, "hegel_zone_source_set_loader")
	if released.Load() != 1 {
		t.Fatalf("released %d times, want 1", released.Load())
	}
	wantErrorContaining(t, bundled.AddOracle(ctx, "Test/Step", stepOracle()), "hegel_zone_source_add_oracle")
}

func TestStubTemporalConstructorErrors(t *testing.T) {
	lib := Stub(t,
		uintptr(0), E_INVALID_ARG, "date options", // date_options_new
		uintptr(0), E_INVALID_ARG, "timezone spec", // timezone_spec_new
		uintptr(0), E_INVALID_ARG, "zoned spec", // zoned_spec_new
		uintptr(0), E_INVALID_ARG, "zone source", // zone_source_new
		uintptr(0), E_INVALID_ARG, "bundled", // zone_source_bundled
		"", E_INVALID_ARG, "format", // format_offset
	)
	if v, err := lib.DateOptionsNew(); v != nil || err == nil {
		t.Fatalf("DateOptionsNew = %v, %v", v, err)
	}
	if v, err := lib.TimezoneSpecNew(); v != nil || err == nil {
		t.Fatalf("TimezoneSpecNew = %v, %v", v, err)
	}
	if v, err := lib.ZonedSpecNew(); v != nil || err == nil {
		t.Fatalf("ZonedSpecNew = %v, %v", v, err)
	}
	if v, err := lib.ZoneSourceNew(); v != nil || err == nil {
		t.Fatalf("ZoneSourceNew = %v, %v", v, err)
	}
	if v, err := lib.ZoneSourceBundled(); v != nil || err == nil {
		t.Fatalf("ZoneSourceBundled = %v, %v", v, err)
	}
	if v, err := lib.FormatOffset(Offset{}); v != "" || err == nil {
		t.Fatalf("FormatOffset = %q, %v", v, err)
	}
}

func TestStubTimezoneInfo(t *testing.T) {
	want := TimezoneInfo{Known: true, PrimaryName: "Asia/Kolkata", IsLink: true, Rank: 7, Tags: 1 << 13}
	lib := Stub(t, want, OK)
	got, err := lib.TimezoneInfoIn(nil, "Asia/Calcutta")
	if err != nil || got != want {
		t.Fatalf("TimezoneInfoIn = %+v, %v; want %+v", got, err, want)
	}
}
