use std::error::Error;
use std::fs::File;
use std::process;

extern crate serde;
use serde::Serialize;

extern crate clap;
use clap::{App, Arg};

#[derive(Serialize)]
struct YnabTransaction {
    import_id: String,
    date: String,
    payee_name: String,
    memo: Option<String>,
    cleared: String,
    amount: i64,
    account_id: String,

    #[serde(skip_serializing)]
    is_commission: bool,
}

impl YnabTransaction {
    fn add_amount(self: Self, commission: i64) -> Self {
        YnabTransaction {
            amount: self.amount + commission,
            ..self
        }
    }
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
        if m.contains(" VALŪTAS KURSS ") && m.contains(" KONVERTĀCIJAS MAKSA ") {
            drop_words(m, " ", 13)
        } else {
            drop_words(m, " ", 6)
        }
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

fn non_empty_field(row: &csv::StringRecord, idx: usize) -> Option<&str> {
    row.get(idx).and_then(|p| Some(p).filter(|p| !p.is_empty()))
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

// YNAB is using a "milliunit" for tx amounts: https://api.youneedabudget.com/#formats
fn fmt_amount(amount: Option<&str>, tx_type: Option<&str>) -> i64 {
    amount
        .and_then(|a| i64::from_str_radix(&a.replace(",", ""), 10).ok())
        .map(|v| match tx_type {
            Some("D") => -10 * v,
            _ => 10 * v,
        })
        .unwrap_or(0)
}

// Returns true if the given transaction contains extra processing fees that need
// to be applied to the previous transaction.
fn is_commission(memo: Option<&str>, op_type: Option<&str>) -> bool {
    match (memo, op_type) {
        (Some(m), Some("KOM")) => m.ends_with(" apkalpošanas komisija"),
        _ => false,
    }
}

fn from_transaction_row(row: &csv::StringRecord, account_id: &str) -> Option<YnabTransaction> {
    let payee = non_empty_field(row, 3);
    let memo = non_empty_field(row, 4);

    match fmt_date(row.get(2), memo) {
        Some(date) => Some(YnabTransaction {
            import_id: row_import_id(&row),
            date,
            payee_name: fmt_payee(payee, memo),
            memo: fmt_memo(payee, memo),
            cleared: String::from("cleared"),
            amount: fmt_amount(row.get(5), row.get(7)),
            account_id: String::from(account_id),
            is_commission: is_commission(memo, row.get(9)),
        }),
        _ => None,
    }
}

fn print_balance(row: &csv::StringRecord) {
    println!(
        "Final balance: {} {}",
        row.get(5).unwrap_or(""),
        row.get(6).unwrap_or("")
    )
}

fn parse_row(row: &csv::StringRecord, account_id: &str) -> Option<YnabTransaction> {
    match row.get(1) {
        Some("20") => from_transaction_row(&row, account_id),
        Some("86") => {
            print_balance(&row);
            None
        }
        _ => None,
    }
}

// HTTP output

#[derive(Serialize)]
struct HttpRequest {
    transactions: Vec<YnabTransaction>,
}

fn request(txns: Vec<YnabTransaction>, args: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let uri = format!(
        "https://api.youneedabudget.com/v1/budgets/{}/transactions",
        args.value_of("budget").unwrap_or("")
    );

    let body = HttpRequest { transactions: txns };

    let client = reqwest::blocking::Client::new();
    let res = client
        .post(&uri)
        .bearer_auth(args.value_of("token").unwrap_or(""))
        .json(&body)
        .send()?;
    println!("{}", res.text()?);
    Ok(())
}

fn is_swedbank_csv(headers: &csv::StringRecord) -> bool {
    if headers.len() == 13 {
        // TODO: once iter_order_by is stable, it can be used here.
        headers
            .iter()
            .take(3)
            .zip(["Klienta konts", "Ieraksta tips", "Datums"].iter())
            .all(|(header, &expected)| header == expected)
    } else {
        false
    }
}

//

fn run(args: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let file = File::open(args.value_of("CSV_PATH").unwrap())?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(b';').from_reader(file);

    let valid_csv = rdr.headers().map(|h| is_swedbank_csv(h)).unwrap_or(false);
    if valid_csv {
        let account_id = args.value_of("account").unwrap_or("");
        let mut txns = rdr
            .records()
            .map(|r| r.ok().and_then(|r| parse_row(&r, account_id)))
            .flatten()
            .peekable();

        // Fold transaction fees into actual purhcase transactions.
        let mut v: Vec<YnabTransaction> = Vec::new();
        loop {
            match (txns.next(), txns.peek()) {
                (Some(t), _) if t.is_commission => continue,
                (Some(t), Some(c)) if c.is_commission => v.push(t.add_amount(c.amount)),
                (Some(t), _) => v.push(t),
                (None, _) => break,
            }
        }
        request(v, args)?;
    } else {
        println!("ERROR: Invalid CSV");
    }

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
    fn test_local_memo_prefix_removal() {
        assert_eq!(
            remove_memo_prefix("PIRKUMS 1 24.02.2020 1.00 EUR (1) CAFE"),
            String::from("CAFE")
        );
    }

    #[test]
    fn test_foreign_memo_prefix_removal() {
        assert_eq!(
            remove_memo_prefix(
                "PIRKUMS 1 17.11.2019 2.50 GBP VALŪTAS KURSS 0.856164, KONVERTĀCIJAS MAKSA 0.06 EUR (1) Rapha"
            ),
            String::from("Rapha")
        )
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
        assert_eq!(fmt_amount(Some("12,99"), Some("D")), -12990);
    }

    #[test]
    fn test_credit_amount() {
        assert_eq!(fmt_amount(Some("0,49"), Some("K")), 490);
    }

    #[test]
    fn test_detect_swed() {
        let headers = csv::StringRecord::from(vec![
            "Klienta konts",
            "Ieraksta tips",
            "Datums",
            "Saņēmējs/Maksātājs",
            "Informācija saņēmējam",
            "Summa",
            "Valūta",
            "Debets/Kredīts",
            "Arhīva kods",
            "Maksājuma veids",
            "Refernces numurs",
            "Dokumenta numurs",
            // XXX: Swedbank includes trailing semicolons,
            //      which are treated like empty columns
            "",
        ]);
        assert_eq!(is_swedbank_csv(&headers), true);
    }

    #[test]
    fn test_detect_swed_invalid_fields() {
        let headers = csv::StringRecord::from(vec![
            "Klienta ponts",
            "Ieraksta tips",
            "Datums",
            "Saņēmējs/Maksātājs",
            "Informācija saņēmējam",
            "Summa",
            "Valūta",
            "Debets/Kredīts",
            "Arhīva kods",
            "Maksājuma veids",
            "Refernces numurs",
            "Dokumenta numurs",
            "",
        ]);
        assert_eq!(is_swedbank_csv(&headers), false);
    }

    #[test]
    fn test_detect_swed_missing_fields() {
        let headers = csv::StringRecord::from(vec![
            "Klienta ponts",
            "Ieraksta tips",
            "Debets/Kredīts",
            "Arhīva kods",
            "Maksājuma veids",
            "Refernces numurs",
            "Dokumenta numurs",
            "",
        ]);
        assert_eq!(is_swedbank_csv(&headers), false);
    }

    #[test]
    fn test_local_tx_fee_memo() {
        assert_eq!(
            is_commission(Some("Maksājumu uzdevuma apkalpošanas komisija"), Some("KOM")),
            true
        );
    }

    #[test]
    fn test_local_tx_no_fee_memo() {
        assert_eq!(
            is_commission(Some("Maksājumu uzdevuma apkalpošanas komisija"), Some("CTX")),
            false
        );
    }

    #[test]
    fn test_international_tx_fee_memo() {
        assert_eq!(
            is_commission(Some("Ārvalstu Maksājumu uzdevumu apkalpošanas komisija"), Some("KOM")),
            true
        );
    }

    #[test]
    fn test_other_kom_tx_memo() {
        assert_eq!(
            is_commission(Some("Kartes mēneša maksa 000000******0000 02.2020"), Some("KOM")),
            false
        );
    }
}
