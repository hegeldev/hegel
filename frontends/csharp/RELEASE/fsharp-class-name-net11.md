RELEASE_TYPE: patch

This patch fixes the class name reported for an F# test on .NET 11, where `Settings.TestLocation.ClassName` was `Hegel.UserCode` rather than the module the test is in.
