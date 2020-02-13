# YNAB API client and CSV converter for Swedbank

`ynab-api` knows both how to transform Swedbank CSVs into CSVs supported by
YNAB & import Swedbank CSVs into YNAB via their API. YNAB API ids can be
either provided via env vars or through command line options.

## Setup

* Install [Rust](https://www.rust-lang.org/learn/get-started)
* `cargo build`

### Usage

```
USAGE:
    ynab-swed [OPTIONS] <CSV_PATH> -a <ACCOUNT> -b <BUDGET> -t <TOKEN> [csv]

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
    <csv>         exports to CSV
```

`ynab-swed` turns this

```csv
"Klienta konts";"Ieraksta tips";"Datums";"Saņēmējs/Maksātājs";"Informācija saņēmējam";"Summa";"Valūta";"Debets/Kredīts";"Arhīva kods";"Maksājuma veids";"Refernces numurs";"Dokumenta numurs";
"LV00HABA0000000000000";"20";"03.09.2019";"A STORE";"PIRKUMS 1234 01.09.2019 1 EUR (10) A STORE";"1,00";"EUR";"D";"0000000000000000";"CTX";"";"";
"LV00HABA0000000000000";"20";"04.09.2019";"JOHN DOE";"Taco Money";"10,65";"EUR";"K";"0000000000000000";"INB";"";"480";
```

Into this

```csv
Id,Date,Payee,Memo,Amount
57ce375b0a2adf54a1426607096eb686,01/09/2019,A STORE,,"-1,00"
05f46d30f5dd8f0cc746fd6290bb1cda,04/09/2019,JOHN DOE,Taco Money,"10,65"
```

## Testing

```
cargo test
```
