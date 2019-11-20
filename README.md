# YNAB CSV converter for Swedbank

## Setup

* Install [Rust](https://www.rust-lang.org/learn/get-started)
* `cargo build`

### Usage

```
ynab-swed [-o formatted-output.cv] raw-input.csv
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
