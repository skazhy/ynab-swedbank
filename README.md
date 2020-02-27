# YNAB API client for Swedbank

`ynab-api` imports Swedbank CSVs into YNAB via their API. YNAB API ids can be
either provided via env vars or through command line options.

Originally this project had YNAB friendly CSV generation from Swedbank CSV statements.
This code is available in [csv-export](https://github.com/skazhy/ynab-swedbank/tree/csv-export) branch.

## Setup

* Install [Rust](https://www.rust-lang.org/learn/get-started)
* `cargo build`
* Generate a [personal YNAB access token](https://app.youneedabudget.com/settings/developer)
* Get your account and budget ids from an account url: `https://app.youneedabudget.com/BUDGET_ID/accounts/ACCOUNT_ID`

## Usage

```
USAGE:
    ynab-swed <CSV_PATH> -a <ACCOUNT> -b <BUDGET> -t <TOKEN>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a <ACCOUNT>         YNAB account id [defaults to env var: YNAB_ACCOUNT]
    -b <BUDGET>          YNAB budget id [defaults to env var: YNAB_BUDGET]
    -t <TOKEN>           YNAB personal access token [defaults to env var YNAB_TOKEN]

ARGS:
    <CSV_PATH>    Path for Swedbank CSV export [defaults to out.csv]
```

## Testing

```
cargo test
```

## Linting and formatting

I'm using [clippy](https://github.com/rust-lang/rust-clippy) for linting &
`rustfmt` for formatting. I'm using this as a pre-commit hook:

```sh
#!/bin/sh

for FILE in `git diff --cached --name-only -- \*.rs`; do
  if ! rustfmt --check -q $FILE > /dev/null; then
    echo "\033[0;31mAborting:\033[0m invalid formatting in: $FILE"
    exit 1
  fi
done
```
