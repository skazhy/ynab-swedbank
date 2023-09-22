# YNAB API client for Swedbank

`ynab-swed` is an opinionated [Swedbank](https://swedbank.com/) account statement importer for [YNAB](https://www.youneedabudget.com/).

## Setup

- Install [Rust](https://www.rust-lang.org/learn/get-started) (Rust 2021 edition is used)
- Run `cargo build`
- Generate a [personal YNAB access token](https://app.youneedabudget.com/settings/developer)
- Get your account and budget ids from an account url: `https://app.youneedabudget.com/BUDGET_ID/accounts/ACCOUNT_ID`

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

`ynab-swed` takes a single account statement CSV file and imports it into the
provided YNAB account. Identifiers (access token, budget and account ids) can be either provided as
env vars or through command line options.

Currency of the destination YNAB account is used & only transactions in that
currency are imported. In the case of multi-currency Swedbank statements,
you'll need to run the script multiple times, with a different budget/account
ids for each currency.

Debug loglevel can be set with `RUST_LOG` env variable, which corresponds to [one of these](https://docs.rs/log/latest/log/enum.Level.html).

## Imported data formatting

Transaction fees are appended to their respective transactions and are not
imported as separate entries.

`ynab-swed` tries it's best to strip [merchants of record](https://www.paddle.com/blog/what-is-merchant-of-record)
from resulting data, so that the actual seller is imported as the payee.
Please open an issue if something is imported in a format you did not expect!

Reference for the input CSV can be found here ([PDF](https://www.swedbank.lv/static/pdf/business/d2d/payments/import/CSVformat_lv.pdf)).
The full spec has not been implemented and only the fields relevant to YNAB
are used.

## Testing, linting, and formatting

Unit tests are run in the standard Rust fashion: `cargo test`.

[clippy](https://github.com/rust-lang/rust-clippy) is used for linting &
`rustfmt` for formatting. Here's a git pre-commit hook that's used for this
project:

```sh
#!/bin/sh

for FILE in `git diff --cached --name-only -- \*.rs`; do
  if ! rustfmt --check -q $FILE > /dev/null; then
    echo "\033[0;31mAborting:\033[0m invalid formatting in: $FILE"
    exit 1
  fi
done
```
