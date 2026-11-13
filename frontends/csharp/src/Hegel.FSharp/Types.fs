namespace Hegel.FSharp

open System

/// Delegates over the functions this library is given, each bound to the function itself. Hegel
/// calls them through <c>UserCode</c>, which the JIT never optimises, so no frame of this library
/// lies between that call and the function: were the function inlined into one, it would vanish
/// from the stack trace and a failure's origin would change part-way through a run.
module internal UserFunction =
    let toFunc (f: 'A -> 'R) : Func<'A, 'R> =
        Delegate.CreateDelegate(typeof<Func<'A, 'R>>, f, "Invoke") :?> Func<'A, 'R>

    let toFunc2 (f: 'A -> 'B -> 'R) : Func<'A, 'B, 'R> =
        Delegate.CreateDelegate(typeof<Func<'A, 'B, 'R>>, OptimizedClosures.FSharpFunc<_, _, _>.Adapt f, "Invoke")
        :?> Func<'A, 'B, 'R>

    let toFunc3 (f: 'A -> 'B -> 'C -> 'R) : Func<'A, 'B, 'C, 'R> =
        Delegate.CreateDelegate(
            typeof<Func<'A, 'B, 'C, 'R>>,
            OptimizedClosures.FSharpFunc<_, _, _, _>.Adapt f,
            "Invoke"
        )
        :?> Func<'A, 'B, 'C, 'R>

type Property = Hegel.Property
type TestCase = Hegel.TestCase
type Settings = Hegel.Settings
type Database = Hegel.Database
type Example = Hegel.Example
type TestLocation = Hegel.TestLocation
type Reporter = Hegel.Reporter
type RunReport = Hegel.RunReport
type RunStatistics = Hegel.RunStatistics
type Failure = Hegel.Failure
type RunStatus = Hegel.RunStatus
type RunErrorKind = Hegel.RunErrorKind
type CaseOutcome = Hegel.CaseOutcome
type Phase = Hegel.Phase
type Verbosity = Hegel.Verbosity
type HealthCheck = Hegel.HealthCheck
type Backend = Hegel.Backend
type NondeterminismStrictness = Hegel.NondeterminismStrictness
type HegelException = Hegel.HegelException
type HegelUsageException = Hegel.HegelUsageException
type HegelRunException = Hegel.HegelRunException
type HealthCheckFailure = Hegel.HealthCheckFailure
type FlakyTestException = Hegel.FlakyTestException
type UnsatisfiableException = Hegel.UnsatisfiableException
type DidNotReproduceException = Hegel.DidNotReproduceException
type InvalidExampleException = Hegel.InvalidExampleException
type MultipleFailuresException = Hegel.MultipleFailuresException
type HegelInfrastructureAttribute = Hegel.HegelInfrastructureAttribute
type Stateful = Hegel.Stateful
type StatefulOptions = Hegel.StatefulOptions
type RuleAttribute = Hegel.RuleAttribute
type InvariantAttribute = Hegel.InvariantAttribute
type Pool<'T> = Hegel.Pool<'T>
type ConcurrentPool<'T> = Hegel.ConcurrentPool<'T>
type Printer = Hegel.Printer
type Document = Hegel.Document
type Label = Hegel.Label
type Dialect = Hegel.Dialect
type Generators = Hegel.Generators
type Generator<'T> = Hegel.Generator<'T>
type PrintableGenerator<'T> = Hegel.PrintableGenerator<'T>
type Deferred<'T> = Hegel.Deferred<'T>
type RecursiveGenerator<'T> = Hegel.RecursiveGenerator<'T>
type RecordGenerator<'T> = Hegel.RecordGenerator<'T>
type ListGenerator<'T> = Hegel.ListGenerator<'T>
type ArrayGenerator<'T> = Hegel.ArrayGenerator<'T>
type SetGenerator<'T> = Hegel.SetGenerator<'T>
type MapGenerator<'Key, 'Value> = Hegel.MapGenerator<'Key, 'Value>
type SampleGenerator<'T> = Hegel.SampleGenerator<'T>
type SubsequenceGenerator<'T> = Hegel.SubsequenceGenerator<'T>
type IntegerGenerator = Hegel.IntegerGenerator
type LongGenerator = Hegel.LongGenerator
type BigIntegerGenerator = Hegel.BigIntegerGenerator
type DoubleGenerator = Hegel.DoubleGenerator
type FloatGenerator = Hegel.FloatGenerator
type TextGenerator = Hegel.TextGenerator
type BinaryGenerator = Hegel.BinaryGenerator
type RegexGenerator = Hegel.RegexGenerator
type LarkGenerator = Hegel.LarkGenerator
type DomainGenerator = Hegel.DomainGenerator
type IpAddressGenerator = Hegel.IpAddressGenerator
type UuidGenerator = Hegel.UuidGenerator
type DateGenerator = Hegel.DateGenerator
type TimeGenerator = Hegel.TimeGenerator
type DateTimeGenerator = Hegel.DateTimeGenerator
type TimeSpanGenerator = Hegel.TimeSpanGenerator
type UtcOffsetGenerator = Hegel.UtcOffsetGenerator
