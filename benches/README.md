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

[![h264-reader - Bencher](https://api.bencher.dev/v0/projects/h264-reader/perf/img?branches=a53abf65-ea6e-482e-8c55-cf6726e77864&testbeds=19d2a260-47fc-44ec-b7f8-314d88408ce7&benchmarks=cc26ec97-55ef-43a5-860f-861ae847d8b3&measures=1dc590ca-477f-4477-8295-672b05d33086&start_time=1706605032000&end_time=1709197032000 "h264-reader")](https://bencher.dev/perf/h264-reader?key=true&reports_per_page=4&branches_per_page=8&testbeds_per_page=8&benchmarks_per_page=8&reports_page=1&branches_page=1&testbeds_page=1&benchmarks_page=1)