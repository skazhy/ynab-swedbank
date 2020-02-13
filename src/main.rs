use std::error::Error;
use std::fs::File;
use std::process;

extern crate serde;
use serde::{Serialize};

extern crate clap;
use clap::{Arg, App};

#[derive(Serialize)]
struct Transaction {
    import_id: String,
    date: String,
    payee: String,
    memo: Option<String>,
    amount: String
}

fn fmt_amount(amount: Option<&str>, tx_type: Option<&str>) -> String {
    // tx_type field 7: "D" (debit) - outbound, "K" (credit) - inbound.
    match tx_type {
        Some("D") => amount.map_or(String::from(""), |v| format!("-{}", &v)),
        Some("K") => String::from(amount.unwrap_or("")),
        _ => String::from("")
    }
}

fn prefixed_memo(v: &str) -> bool {
    v.starts_with("PIRKUMS ")
}

/// Splits the string with given splitter, drops n first items
/// at joins the string back together
fn drop_words(s: &str, splitter: &str, n: usize) -> String {
    s.split(splitter).skip(n)
        .filter(|x| !x.is_empty())
        .collect::<Vec<&str>>().join(splitter)
}

fn remove_memo_prefix(m: &str) -> String {
    if prefixed_memo(m) { drop_words(m, " ", 6) }
    else { String::from(m) }.replace("'", "")
}

/// Formats the memo, removes duplicate payee information
fn fmt_memo(payee: Option<&str>, memo: Option<&str>) -> Option<String> {
    memo.map(|m| remove_memo_prefix(m))
        .and_then(|m|
            if payee.map_or(false, |p| m.starts_with(p)) { None }
            else { Some(m) })
}

/// Formats the payee, defaults to "Swedbank" if the field is empty.
fn fmt_payee(payee: Option<&str>, memo: Option<&str>) -> String {
    match (payee, memo) {
        (Some("SumUp"), Some(m)) => drop_words(m, "*", 1),
        (Some(p), _)  => String::from(p.replace("'", "").trim_start_matches("IZ *")),
        _ => String::from("Swedbank")
    }
}

/// Extracts the actual transaction date (MM.DD.YYYY) from the memo string.
fn extract_transaction_date(memo: Option<&str>) -> Option<&str> {
    memo.and_then(|v| if prefixed_memo(v) { v.split(" ").nth(2) } else { None })
}

fn fmt_date(date: Option<&str>, memo: Option<&str>) -> String {
    match extract_transaction_date(memo).or(date) {
        Some(d) => d.replace(".", "/"),
        None => String::from("")
    }
}

fn row_import_id(row: &csv::StringRecord) -> String {
    // import id = md5 hash of the following:
    // raw date(2) payee(3) description (4) amount (5) doc. number (11)
    let ids = [2, 3, 4, 5, 11];
    let digest = md5::compute(
        ids.iter().fold("".to_string(), |mut acc, id| {
            acc.push_str(row.get(*id).unwrap_or(""));
            acc.push_str("|");
            acc
        })
    );
    format!("{:x}", digest)
}

fn create_transaction(row: &csv::StringRecord) -> Option<Transaction> {
    let payee = row.get(3).and_then(|p| if p.is_empty() { None } else { Some(p) });
    let memo = row.get(4).and_then(|m| if m.is_empty() { None } else { Some(m) });

    match row.get(1) {
        Some("20") => Some(Transaction {
            import_id: row_import_id(&row),
            date: fmt_date(row.get(2), memo),
            payee: fmt_payee(payee, memo),
            memo: fmt_memo(payee, memo),
            amount: fmt_amount(row.get(5), row.get(7))
        }),
        _ => None
    }
}

fn run(args: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let file = File::open(args.value_of("CSV_PATH").unwrap())?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(file);

    let mut wtr = csv::Writer::from_path(args.value_of("output").unwrap_or("out.csv"))?;

    for result in rdr.records() {
        let row = result?;
        match create_transaction(&row) {
            Some(t) => wtr.serialize(t)?,
            _ => continue
        };
    };
    wtr.flush()?;
    Ok(())
}

fn main() {
    let args = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(Arg::with_name("CSV_PATH")
            .help("Path for Swedbank CSV export")
            .required(true))
        .arg(Arg::with_name("output")
            .short("o")
            .value_name("OUT_PATH")
            .help("CSV Output path (defaults to out.csv)"))
        .get_matches();

    if let Err(err) = run(args) {
        println!("{}", err);
        process::exit(1);
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_absent_payee() {
        assert_eq!(fmt_payee(None, Some("Payment")), "Swedbank");
    }

    #[test]
    fn test_sumup_payee() {
        assert_eq!(fmt_payee(Some("SumUp"), Some("SumUp  *Foobar 1")), "Foobar 1");
    }

    #[test]
    fn test_izettle_payee() {
        assert_eq!(fmt_payee(Some("IZ *Payee222"), Some("memo!")), "Payee222");
    }

    #[test]
    fn test_escapable_payee() {
        assert_eq!(fmt_payee(Some("'Foobar"), Some("Test")), "Foobar");
    }

    #[test]
    fn test_memo_tx_date() {
        assert_eq!(
            extract_transaction_date(
                Some("PIRKUMS 1234 07.07.2019 1.00 EUR (975255) RIMI")),
                Some("07.07.2019")
        );
    }

    #[test]
    fn test_no_payee_memo() {
        assert_eq!(fmt_memo(None, Some("Memo!")), Some(String::from("Memo!")));
    }

    #[test]
    fn test_discard_memo() {
        assert_eq!(fmt_memo(Some("Payee"), Some("Payee TX1")), None);
    }

    #[test]
    fn test_memo_no_tx() {
        assert_eq!(extract_transaction_date(Some("Cash Money")), None);
        assert_eq!(extract_transaction_date(None), None);
    }

    #[test]
    fn test_tx_date() {
        assert_eq!(fmt_date(Some("2019.01.01"), Some("Cash Money")), String::from("2019/01/01"));
        assert_eq!(fmt_date(Some("2019.01.01"), None), String::from("2019/01/01"));
    }

    #[test]
    fn test_debit_amount() {
        assert_eq!(fmt_amount(Some("10.00"), Some("D")), "-10.00");
    }

    #[test]
    fn test_credit_amount() {
        assert_eq!(fmt_amount(Some("10.00"), Some("K")), "10.00");
    }
}
