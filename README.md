# pms-master

## Test

### A+B

This test is based on an A+B problem.

| case | stdin | stdout |
|:-:|:-:|:-:|
| #1   | [1.in](./assets/stdin/1.in)  | [1.out](./assets/stdout/1.out)  |

| test | command | source |
|:--:|:------:|:----:|
| AC1 | `cargo test -- --nocapture test_ac1` | [ac_1.cpp](./assets/cpp/ac_1.cpp) |
| TLE1 | `cargo test -- --nocapture test_tle1` | [tle_1.cpp](./assets/cpp/tle_1.cpp) |
| RTE1 | `cargo test -- --nocapture test_rte1` | [rte_1.cpp](./assets/cpp/rte_1.cpp) |

### IOI 2017 P4 - The Big Prize

| case | stdin | stdout |
|:-:|:-:|:-:|
| #1   | [1.in](./assets/prize/stdin/1.in)  | [1.out](./assets/prize/stdout/1.out)  |

| test | command | source |
|:--:|:------:|:----:|
| AC | `cargo test -- --nocapture test_ac_prize` | [ac_optimal.cpp](./assets/prize/cpp/ac_optimal.cpp) |

## TODO

## License

- [lcmp.cpp](./assets/checker/lcmp.cpp) by MikeMirzayanov in [MIT License](https://opensource.org/licenses/MIT)
- [IOI 2017 P4 - The Big Prize](./assets/prize) by IOI 2017 Committee, available in [link](https://ioi2017.ir/contest/tasks/)