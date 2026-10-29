using System.Reflection;
using System.Reflection.Emit;
using Xunit;

namespace Hegel.Tests;

public class CallerLocationTests
{
    private static void StaticBody(TestCase tc) { }

    private sealed class Nested
    {
        internal Action<TestCase> Lambda() => tc => { };

        internal Action<TestCase> Local()
        {
            void Body(TestCase tc) { }
            return Body;
        }

        internal Action<TestCase> Closure()
        {
            var captured = 1;
            return tc => captured++;
        }
    }

    [Fact]
    public void ClassNameOfAMethodGroupIsItsDeclaringType()
    {
        Assert.Equal(typeof(CallerLocationTests).FullName, CallerLocation.ClassName(StaticBody));
    }

    [Fact]
    public void ClassNameOfALambdaWalksOutOfTheCompilerGeneratedType()
    {
        Assert.Equal(typeof(Nested).FullName, CallerLocation.ClassName(new Nested().Lambda()));
        Assert.Equal(typeof(Nested).FullName, CallerLocation.ClassName(new Nested().Local()));
        Assert.Equal(typeof(Nested).FullName, CallerLocation.ClassName(new Nested().Closure()));
    }

    [Fact]
    public void ClassNameLooksThroughAWrapperFromAnInfrastructureAssembly()
    {
        var marker = typeof(HegelInfrastructureAttribute).GetConstructor([])!;
        var assembly = AssemblyBuilder.DefineDynamicAssembly(new AssemblyName("Hegel.Tests.Wrapping"), AssemblyBuilderAccess.Run, [new CustomAttributeBuilder(marker, [])]);
        var builder = assembly.DefineDynamicModule("m").DefineType("Wrapper");
        var first = builder.DefineField("first", typeof(object), FieldAttributes.Public);
        var second = builder.DefineField("second", typeof(object), FieldAttributes.Public);
        var run = builder.DefineMethod("Run", MethodAttributes.Public, typeof(void), [typeof(TestCase)]);
        run.GetILGenerator().Emit(OpCodes.Ret);
        var wrapper = builder.CreateType();
        var target = Activator.CreateInstance(wrapper)!;
        var body = wrapper.GetMethod("Run")!.CreateDelegate<Action<TestCase>>(target);

        Assert.Equal("Wrapper", CallerLocation.ClassName(body));
        wrapper.GetField("first")!.SetValue(target, "a captured string");
        wrapper.GetField("second")!.SetValue(target, new Nested());
        Assert.Equal(typeof(Nested).FullName, CallerLocation.ClassName(body));
        wrapper.GetField("second")!.SetValue(target, new Nested().Lambda());
        Assert.Equal(typeof(Nested).FullName, CallerLocation.ClassName(body));
    }

    [Fact]
    public void ClassNameOfAnAnonymousDelegateIsEmpty()
    {
        var method = new System.Reflection.Emit.DynamicMethod("anon", typeof(void), [typeof(TestCase)]);
        var il = method.GetILGenerator();
        il.Emit(System.Reflection.Emit.OpCodes.Ret);
        var body = (Action<TestCase>)method.CreateDelegate(typeof(Action<TestCase>));
        Assert.Equal("", CallerLocation.ClassName(body));
    }

    [Fact]
    public void DatabaseKeyJoinsFileAndFunction()
    {
        Assert.Equal("a/b.cs::Method", CallerLocation.DatabaseKey(new TestLocation("a/b.cs", 1, "C", "Method")));
    }

    [Fact]
    public void ProjectPathOfAnEmptyFileIsEmpty()
    {
        Assert.Equal("", CallerLocation.ProjectPath(""));
    }

    [Fact]
    public void ProjectPathIsRelativeToTheRepositoryRoot()
    {
        using var root = new TempDir();
        Directory.CreateDirectory(Path.Combine(root.Path, ".git"));
        var file = Path.Combine(root.Path, "src", "Deep", "File.cs");
        Directory.CreateDirectory(Path.GetDirectoryName(file)!);
        File.WriteAllText(file, "");
        Assert.Equal("src/Deep/File.cs", CallerLocation.ProjectPath(file));
    }

    [Fact]
    public void ProjectPathRecognisesAGitFileAndASolution()
    {
        using var root = new TempDir();
        File.WriteAllText(Path.Combine(root.Path, ".git"), "gitdir: elsewhere");
        var file = Path.Combine(root.Path, "a", "File.cs");
        Directory.CreateDirectory(Path.GetDirectoryName(file)!);
        Assert.Equal("a/File.cs", CallerLocation.ProjectPath(file));

        using var sln = new TempDir();
        File.WriteAllText(Path.Combine(sln.Path, "Thing.SLN"), "");
        Directory.CreateDirectory(Path.Combine(sln.Path, "b"));
        Assert.Equal("b/File.cs", CallerLocation.ProjectPath(Path.Combine(sln.Path, "b", "File.cs")));

        using var slnx = new TempDir();
        File.WriteAllText(Path.Combine(slnx.Path, "Thing.slnx"), "");
        Assert.Equal("File.cs", CallerLocation.ProjectPath(Path.Combine(slnx.Path, "File.cs")));
    }

    [Fact]
    public void ProjectPathSkipsDirectoriesThatDoNotExist()
    {
        using var root = new TempDir();
        Directory.CreateDirectory(Path.Combine(root.Path, ".git"));
        Assert.Equal("missing/File.cs", CallerLocation.ProjectPath(Path.Combine(root.Path, "missing", "File.cs")));
    }

    [Fact]
    public void ProjectPathFallsBackToTheFileNameWithoutARoot()
    {
        Assert.Null(CallerLocation.ProjectRoot(null));
        Assert.Equal("File.cs", CallerLocation.ProjectPath(Path.Combine(Path.GetPathRoot(Path.GetTempPath())!, "no-such-dir-hegel", "File.cs")));
    }

    [Fact]
    public void OfCombinesEverything()
    {
        var location = CallerLocation.Of(StaticBody, "", 12, "Member");
        Assert.Equal(new TestLocation("", 12, typeof(CallerLocationTests).FullName!, "Member"), location);
    }

    private sealed class TempDir : IDisposable
    {
        internal string Path { get; } = Directory.CreateTempSubdirectory("hegel-caller-").FullName;

        public void Dispose() => Directory.Delete(Path, true);
    }
}
