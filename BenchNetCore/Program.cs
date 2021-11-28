using BenchmarkDotNet.Running;
using System;

namespace BenchNetCore
{
    class Program
    {
        static void Main(string[] args)
        {
            var summary = BenchmarkRunner.Run<BenchCasts>();
        }
    }
}
