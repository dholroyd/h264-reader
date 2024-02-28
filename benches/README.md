# Benchmarks

There are two benchmarks here,

 - `bench.rs`
   - [criterion.rs](https://github.com/bheisler/criterion.rs) based benchmark suite that measures throughput of various
     h264 parsing setups, reporting on parser throughput (i.e. number of megabytes of h264 data parsed per second).
   - Provides a view of how fast the parser is on the system on which the benchmark is executed.
   - Run using `cargo criterion --bench bench`
 - `bench-ci.rs`
   - [iai-callgrind](https://github.com/iai-callgrind/iai-callgrind) based benchmark that counts the number of
     instructions executed while parsing a test asset
   - Provides an indication of performance that is stable even if run on CPUs of varying speeds, or in the face of
     CPU contention from other processes (e.g. in a noisy CI environment like github actions).
   - Useful for comparing changes in performance over time, not useful for indicating _absolute_ performance.
   - run using `cargo bench --bench ci_bench`

The latter benchmark is run from a github actions workflow on commits to the main branch and the benchmark results are
uploaded to [bencher.dev](https://bencher.dev/perf/h264-reader).