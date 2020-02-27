use std::error::Error;
use std::fs::File;
use std::process;
use std::str::FromStr;

extern crate rust_decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

extern crate serde;
use serde::Serialize;

extern crate clap;
use clap::{App, Arg};

#[derive(Clone)]
struct Transaction {
    import_id: String,
    date: String,
    payee: String,
    memo: Option<String>,
    amount: Decimal,
}

fn fmt_amount(amount: Option<&str>, tx_type: Option<&str>) -> Option<Decimal> {
    amount
        .map(|a| Decimal::from_str(&a.replace(",", ".")))
        .and_then(|res| res.ok())
        .map(|v| match tx_type {
            Some("D") => -v,
            _ => v,
        })
}

fn prefixed_memo(v: &str) -> bool {
    v.starts_with("PIRKUMS ")
}

/// Splits the string with given splitter, drops n first items
/// at joins the string back together
fn drop_words(s: &str, splitter: &str, n: usize) -> String {
    s.split(splitter)
        .skip(n)
        .filter(|x| !x.is_empty())
        .collect::<Vec<&str>>()
        .join(splitter)
}

fn remove_memo_prefix(m: &str) -> String {
    if prefixed_memo(m) {
        drop_words(m, " ", 6)
    } else {
        String::from(m)
    }
    .replace("'", "")
}

/// Formats the memo, removes duplicate payee information
fn fmt_memo(payee: Option<&str>, memo: Option<&str>) -> Option<String> {
    memo.map(|m| remove_memo_prefix(m)).and_then(|m| {
        if payee.map_or(false, |p| m.starts_with(p)) {
            None
        } else {
            Some(m)
        }
    })
}

/// Formats the payee, defaults to "Swedbank" if the field is empty.
fn fmt_payee(payee: Option<&str>, memo: Option<&str>) -> String {
    match (payee, memo) {
        (Some("SumUp"), Some(m)) => drop_words(m, "*", 1),
        (Some(p), _) => String::from(p.replace("'", "").trim_start_matches("IZ *")),
        _ => String::from("Swedbank"),
    }
}

/// Extracts the actual transaction date (MM.DD.YYYY) from the memo string.
fn extract_transaction_date(memo: Option<&str>) -> Option<&str> {
    memo.and_then(|v| if prefixed_memo(v) { v.split(' ').nth(2) } else { None })
}

fn reorder_date(d: &str) -> String {
    let mut parts = d.split('.').collect::<Vec<&str>>();
    parts.reverse();
    parts.join("-")
}

fn fmt_date(date: Option<&str>, memo: Option<&str>) -> Option<String> {
    extract_transaction_date(memo).or(date).map(|d| reorder_date(d))
}

fn row_import_id(row: &csv::StringRecord) -> String {
    // import id = md5 hash of the following:
    // raw date(2) payee(3) description (4) amount (5) doc. number (11)
    let ids = [2, 3, 4, 5, 11];
    let digest = md5::compute(ids.iter().fold("".to_string(), |mut acc, id| {
        acc.push_str(row.get(*id).unwrap_or(""));
        acc.push_str("|");
        acc
    }));
    format!("{:x}", digest)
}

fn from_transaction_row(row: &csv::StringRecord) -> Option<Transaction> {
    let payee = row.get(3).and_then(|p| if p.is_empty() { None } else { Some(p) });
    let memo = row.get(4).and_then(|m| if m.is_empty() { None } else { Some(m) });

    match (fmt_amount(row.get(5), row.get(7)), fmt_date(row.get(2), memo)) {
        (Some(amount), Some(date)) => Some(Transaction {
            import_id: row_import_id(&row),
            date,
            payee: fmt_payee(payee, memo),
            memo: fmt_memo(payee, memo),
            amount,
        }),
        _ => None,
    }
}

fn parse_row(row: &csv::StringRecord) -> Option<Transaction> {
    match row.get(1) {
        Some("20") => from_transaction_row(&row),
        _ => None,
    }
}

// HTTP output

#[derive(Serialize)]
struct HttpRequest {
    transactions: Vec<HttpTransaction>,
}

#[derive(Serialize)]
struct HttpTransaction {
    import_id: String,
    date: String,
    payee_name: String,
    memo: Option<String>,
    // a "milliunit" is used in transactions: https://api.youneedabudget.com/#formats
    cleared: String,
    amount: i64,
    account_id: String,
}

fn from_transaction(tx: Transaction, account_id: &str) -> HttpTransaction {
    HttpTransaction {
        import_id: tx.import_id,
        date: tx.date,
        payee_name: tx.payee,
        memo: tx.memo,
        cleared: String::from("cleared"),
        amount: (tx.amount * Decimal::new(1000, 0)).to_i64().unwrap_or(0),
        account_id: account_id.to_string(),
    }
}

fn request(txns: Vec<Transaction>, args: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let account_id = args.value_of("account").unwrap_or("");
    let uri = format!(
        "https://api.youneedabudget.com/v1/budgets/{}/transactions",
        args.value_of("budget").unwrap_or("")
    );

    let body = HttpRequest {
        transactions: txns.iter().cloned().map(|t| from_transaction(t, account_id)).collect(),
    };

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(&uri)
        .bearer_auth(args.value_of("token").unwrap_or(""))
        .json(&body)
        .send()?;
    println!("{}", res.text()?);
    Ok(())
}

//

fn run(args: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let file = File::open(args.value_of("CSV_PATH").unwrap())?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(b';').from_reader(file);
    let txns = rdr
        .records()
        .map(|r| r.ok().and_then(|r| parse_row(&r)))
        .flatten()
        .collect();
    request(txns, args)?;
    Ok(())
}

fn main() {
    let args = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("CSV_PATH")
                .help("Path for Swedbank CSV export")
                .required(true),
        )
        .arg(
            Arg::with_name("token")
                .short("t")
                .required(true)
                .env("YNAB_TOKEN")
                .value_name("TOKEN")
                .help("YNAB personal acces token"),
        )
        .arg(
            Arg::with_name("budget")
                .short("b")
                .required(true)
                .env("YNAB_BUDGET")
                .value_name("BUDGET")
                .help("YNAB budget id"),
        )
        .arg(
            Arg::with_name("account")
                .short("a")
                .required(true)
                .env("YNAB_ACCOUNT")
                .value_name("ACCOUNT")
                .help("YNAB account id"),
        )
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
            extract_transaction_date(Some("PIRKUMS 1234 07.07.2019 1.00 EUR (975255) RIMI")),
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
        assert_eq!(
            fmt_date(Some("09.02.2020"), Some("Cash Money")),
            Some(String::from("2020-02-09"))
        );
        assert_eq!(fmt_date(Some("09.02.2020"), None), Some(String::from("2020-02-09")));
    }

    #[test]
    fn test_debit_amount() {
        assert_eq!(fmt_amount(Some("12,99"), Some("D")), Some(Decimal::new(-1299, 2)));
    }

    #[test]
    fn test_credit_amount() {
        assert_eq!(fmt_amount(Some("0,49"), Some("K")), Some(Decimal::new(49, 2)));
    }
}
