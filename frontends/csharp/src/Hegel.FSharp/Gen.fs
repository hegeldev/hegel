namespace Hegel.FSharp

open System
open System.Runtime.CompilerServices
open Hegel

/// The generator catalogue for F#. The scalar generators are the C# ones, whose builder methods
/// (<c>Gen.int.Min(0).Max(10)</c>) narrow them; the collection, option, tuple, record and union
/// generators are F#'s own, drawing F# values and printing them as F#. Generators compose with
/// <see cref="map"/>, <see cref="filter"/> and <see cref="bind"/>, or in a <c>gen { }</c> block.
[<RequireQualifiedAccess>]
module Gen =
    /// A generator of just <paramref name="value"/>.
    let constant (value: 'T) : Generator<'T> = Generators.Just value

    /// A generator choosing uniformly among <paramref name="values"/>.
    let item (values: seq<'T>) : Generator<'T> = Generators.SampledFrom values

    /// A generator drawing from one of <paramref name="generators"/>, chosen uniformly.
    let oneOf (generators: seq<Generator<'T>>) : Generator<'T> =
        Generators.OneOf(Array.ofSeq generators)

    /// The values of <paramref name="generator"/> transformed by <paramref name="f"/>.
    let map (f: 'T -> 'U) (generator: Generator<'T>) : Generator<'U> = generator.Map(Func<'T, 'U> f)

    /// The values of <paramref name="generator"/> that <paramref name="predicate"/> accepts.
    let filter (predicate: 'T -> bool) (generator: Generator<'T>) : Generator<'T> =
        generator.Filter(Func<'T, bool> predicate)

    /// A value of <paramref name="generator"/>, then one of the generator <paramref name="f"/> gives for it.
    let bind (f: 'T -> Generator<'U>) (generator: Generator<'T>) : Generator<'U> =
        generator.FlatMap(Func<'T, Generator<'U>> f)

    /// Options: <c>None</c> or <c>Some</c> of a value of <paramref name="generator"/>.
    let option (generator: Generator<'T>) : OptionGen<'T> = OptionGen<'T>(generator)

    /// Value options: <c>ValueNone</c> or <c>ValueSome</c> of a value of <paramref name="generator"/>.
    let voption (generator: Generator<'T>) : ValueOptionGen<'T> = ValueOptionGen<'T>(generator)

    /// Lists of values of <paramref name="element"/>; <c>.MinSize</c>, <c>.MaxSize</c> and <c>.Unique</c> narrow them.
    let list (element: Generator<'T>) : ListGen<'T> =
        ListGen<'T>(element, 0, Native.Abi.Unbounded, None)

    /// Arrays of values of <paramref name="element"/>; <c>.MinSize</c>, <c>.MaxSize</c> and <c>.Unique</c> narrow them.
    let array (element: Generator<'T>) : ArrayGen<'T> =
        ArrayGen<'T>(element, 0, Native.Abi.Unbounded, None)

    /// Sets of values of <paramref name="element"/>; <c>.MinSize</c> and <c>.MaxSize</c> narrow them.
    let set (element: Generator<'T>) : SetGen<'T> =
        SetGen<'T>(element, 0, Native.Abi.Unbounded)

    /// Maps with keys from <paramref name="keys"/> and values from <paramref name="values"/>.
    let mapOf (keys: Generator<'Key>) (values: Generator<'Value>) : MapGen<'Key, 'Value> =
        MapGen<'Key, 'Value>(keys, values, 0, Native.Abi.Unbounded)

    [<MethodImpl(MethodImplOptions.NoInlining)>]
    let private tuple (elements: IGenerator[]) (make: obj[] -> 'T) =
        TupleGen<'T>(elements, make, "(") :> Generator<'T>

    /// Pairs, one element from each generator.
    let tuple2 (a: Generator<'A>) (b: Generator<'B>) : Generator<'A * 'B> =
        tuple [| a; b |] (fun values -> unbox values[0], unbox values[1])

    /// Triples, one element from each generator.
    let tuple3 (a: Generator<'A>) (b: Generator<'B>) (c: Generator<'C>) : Generator<'A * 'B * 'C> =
        tuple [| a; b; c |] (fun values -> unbox values[0], unbox values[1], unbox values[2])

    /// Quadruples, one element from each generator.
    let tuple4
        (a: Generator<'A>)
        (b: Generator<'B>)
        (c: Generator<'C>)
        (d: Generator<'D>)
        : Generator<'A * 'B * 'C * 'D> =
        tuple [| a; b; c; d |] (fun values -> unbox values[0], unbox values[1], unbox values[2], unbox values[3])

    /// Quintuples, one element from each generator.
    let tuple5
        (a: Generator<'A>)
        (b: Generator<'B>)
        (c: Generator<'C>)
        (d: Generator<'D>)
        (e: Generator<'E>)
        : Generator<'A * 'B * 'C * 'D * 'E> =
        tuple [| a; b; c; d; e |] (fun values ->
            unbox values[0], unbox values[1], unbox values[2], unbox values[3], unbox values[4])

    /// Pairs, one element from each generator: <see cref="tuple2"/>.
    let zip (a: Generator<'A>) (b: Generator<'B>) : Generator<'A * 'B> = tuple2 a b

    /// <paramref name="f"/> applied to a value from each generator.
    let map2 (f: 'A -> 'B -> 'T) (a: Generator<'A>) (b: Generator<'B>) : Generator<'T> =
        tuple2 a b |> map (fun (x, y) -> f x y)

    /// <paramref name="f"/> applied to a value from each generator.
    let map3 (f: 'A -> 'B -> 'C -> 'T) (a: Generator<'A>) (b: Generator<'B>) (c: Generator<'C>) : Generator<'T> =
        tuple3 a b c |> map (fun (x, y, z) -> f x y z)

    /// A generator written as a function of the test case, drawing what it needs; its value is printed.
    let composite (body: TestCase -> 'T) : Generator<'T> =
        Generators.Composite(Func<TestCase, 'T> body)

    /// Recursive values: <paramref name="baseCase"/>, extended by <paramref name="extend"/> a bounded
    /// number of times; <c>.MaxDepth</c> and <c>.MaxLeaves</c> set the bounds.
    let recursive (baseCase: Generator<'T>) (extend: Generator<'T> -> Generator<'T>) : RecursiveGenerator<'T> =
        Generators.Recursive(baseCase, Func<Generator<'T>, Generator<'T>> extend)

    /// A generator set later with <c>.Set</c>, for one that refers to itself.
    let deferred () : Deferred<'T> = Generators.Deferred<'T>()

    /// A generator derived from the type: F# records, unions (recursive ones through their base
    /// cases), options, lists, arrays, sets, maps and tuples, recursively, and every type
    /// <c>Generators.ForType</c> knows. Derived once per type.
    [<GeneralizableValue; RequiresExplicitTypeArguments>]
    let auto<'T> : Generator<'T> = Auto.forType typeof<'T> :?> Generator<'T>

    /// A generator for the record <typeparamref name="T"/>, each field derived from its type unless
    /// <c>.With("Field", generator)</c> says otherwise.
    [<GeneralizableValue; RequiresExplicitTypeArguments>]
    let record<'T> : RecordGen<'T> = RecordGen<'T>.Derived Auto.forType

    /// Permutations of <paramref name="items"/>.
    let permutations (items: seq<'T>) : Generator<'T list> =
        Generators.Permutations items |> map List.ofSeq

    /// Subsequences of <paramref name="items"/>, in their order.
    let subsequences (items: seq<'T>) : Generator<'T list> =
        Generators.Subsequences items |> map List.ofSeq

    /// Samples of <paramref name="items"/>: subsets of any size, in a drawn order.
    let samples (items: seq<'T>) : Generator<'T list> =
        Generators.Samples items |> map List.ofSeq

    /// Integers (32-bit); <c>.Min</c> and <c>.Max</c> bound them.
    let int: IntegerGenerator = Generators.Integers()

    /// Integers (64-bit); <c>.Min</c> and <c>.Max</c> bound them.
    let int64: LongGenerator = Generators.Longs()

    /// Arbitrary-precision integers; <c>.Min</c> and <c>.Max</c> bound them.
    let bigint: BigIntegerGenerator = Generators.BigIntegers()

    /// Floating-point numbers (64-bit); <c>.Min</c>, <c>.Max</c>, <c>.AllowNan</c> and <c>.AllowInfinity</c> narrow them.
    let float: DoubleGenerator = Generators.Doubles()

    /// Floating-point numbers (32-bit); <c>.Min</c>, <c>.Max</c>, <c>.AllowNan</c> and <c>.AllowInfinity</c> narrow them.
    let float32: FloatGenerator = Generators.Floats()

    /// Booleans, true and false equally often.
    let bool: Generator<bool> = Generators.Booleans()

    /// Booleans, true with probability <paramref name="p"/>.
    let weightedBool (p: float) : Generator<bool> = Generators.WeightedBooleans p

    /// Strings; <c>.MinSize</c>, <c>.MaxSize</c>, <c>.Alphabet</c> and <c>.Categories</c> narrow them.
    let string: TextGenerator = Generators.Text()

    /// Characters other than surrogates.
    let char: Generator<char> =
        Generators.Integers().Min(0).Max(Operators.int Char.MaxValue)
        |> filter (fun c -> not (Char.IsSurrogate(Operators.char c)))
        |> map Operators.char

    /// Byte arrays; <c>.MinSize</c> and <c>.MaxSize</c> bound their length.
    let bytes: BinaryGenerator = Generators.Binary()

    /// Strings matching <paramref name="pattern"/>; <c>.FullMatch</c> and <c>.Alphabet</c> narrow them.
    let regex (pattern: string) : RegexGenerator = Generators.FromRegex pattern

    /// Strings of the Lark <paramref name="grammar"/>; <c>.Start</c>, <c>.MaxSize</c> and <c>.MaxDepth</c> narrow them.
    let lark (grammar: string) : LarkGenerator = Generators.FromLark grammar

    /// Email addresses.
    let email: Generator<string> = Generators.Emails()

    /// URLs.
    let url: Generator<string> = Generators.Urls()

    /// Domain names; <c>.MaxLength</c> bounds them.
    let domain: DomainGenerator = Generators.Domains()

    /// IP addresses; <c>.V4</c> and <c>.V6</c> pick a version.
    let ipAddress: IpAddressGenerator = Generators.IpAddresses()

    /// GUIDs; <c>.Version</c> picks a UUID version.
    let guid: UuidGenerator = Generators.Uuids()

    /// Dates; <c>.Min</c> and <c>.Max</c> bound them.
    let dateOnly: DateGenerator = Generators.Dates()

    /// Times of day; <c>.Min</c> and <c>.Max</c> bound them.
    let timeOnly: TimeGenerator = Generators.Times()

    /// Date-times; <c>.Min</c>, <c>.Max</c>, <c>.WithTimeZones</c> and <c>.WithOffsets</c> narrow them.
    let dateTime: DateTimeGenerator = Generators.DateTimes()

    /// Date-times with a UTC offset.
    let dateTimeOffset: Generator<DateTimeOffset> =
        Generators.DateTimes().WithOffsets(Generators.UtcOffsets())

    /// Durations; <c>.Min</c> and <c>.Max</c> bound them.
    let timeSpan: TimeSpanGenerator = Generators.TimeSpans()

    /// UTC offsets; <c>.Min</c> and <c>.Max</c> bound them.
    let utcOffset: UtcOffsetGenerator = Generators.UtcOffsets()

    /// The system's time zones.
    let timeZone: Generator<TimeZoneInfo> = Generators.TimeZones()

/// The <c>gen { }</c> block: <c>let!</c> draws from a generator and <c>return</c> gives the value,
/// which is printed as the block's draw.
type GenBuilder() =
    /// A draw from <paramref name="generator"/>, continued by <paramref name="f"/>.
    member _.Bind(generator: Generator<'T>, f: 'T -> Generator<'U>) : Generator<'U> = Gen.bind f generator

    /// The block's value.
    member _.Return(value: 'T) : Generator<'T> = Gen.constant value

    /// The block's value, drawn from <paramref name="generator"/>.
    member _.ReturnFrom(generator: Generator<'T>) : Generator<'T> = generator

    /// A draw from <paramref name="generator"/>, transformed by <paramref name="f"/> into the block's value.
    member _.BindReturn(generator: Generator<'T>, f: 'T -> 'U) : Generator<'U> = Gen.map f generator

    /// The block, built when the generator is.
    member _.Delay(f: unit -> Generator<'T>) : Generator<'T> = f ()

[<AutoOpen>]
module GenBuilder =
    /// The <c>gen { }</c> block.
    let gen = GenBuilder()
