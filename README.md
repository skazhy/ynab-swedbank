# YNAB API client Swedbank

`ynab-api` knows import Swedbank CSVs into YNAB via their API. YNAB API ids can be
either provided via env vars or through command line options.

Originally this project had YNAB friendly CSV generation from Swedbank CSV statements.
This code is available in [csv-export](https://github.com/skazhy/ynab-swedbank/tree/csv-export) branch.

## Setup

* Install [Rust](https://www.rust-lang.org/learn/get-started)
* `cargo build`

### Usage

```
USAGE:
    ynab-swed [OPTIONS] <CSV_PATH> -a <ACCOUNT> -b <BUDGET> -t <TOKEN>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a <ACCOUNT>         YNAB account id [defaults to env var: YNAB_ACCOUNT]
    -b <BUDGET>          YNAB budget id [defaults to env var: YNAB_BUDGET]
    -o <OUT_PATH>        CSV Output path (defaults to out.csv)
    -t <TOKEN>           YNAB personal access token [defaults to env var YNAB_TOKEN]

ARGS:
    <CSV_PATH>    Path for Swedbank CSV export [defaults to out.csv]
```

## Testing

```
cargo test
```
