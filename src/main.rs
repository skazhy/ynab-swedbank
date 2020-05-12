use std::error::Error;
use std::fs::File;
use std::process;

extern crate serde;
use serde::Serialize;

extern crate clap;
use clap::{App, Arg};

mod swed;
use swed::*;

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
    needs_rollup: bool,
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
fn fmt_memo(payee: &str, memo: &str) -> Option<String> {
    let m2 = remove_memo_prefix(memo);
    if !payee.is_empty() && m2.starts_with(payee) {
        None
    } else {
        Some(m2)
    }
}

/// Formats the payee, defaults to "Swedbank" if the field is empty.
fn fmt_payee(payee: &str, memo: &str) -> String {
    match payee {
        "" => String::from("Swedbank"),
        "SumUp" => drop_words(memo, "*", 1),
        p => String::from(p.replace("'", "").trim_start_matches("IZ *")),
    }
}

/// Extracts the actual transaction date (MM.DD.YYYY) from the memo string.
fn extract_transaction_date(memo: &str) -> Option<&str> {
    if prefixed_memo(memo) {
        memo.split(' ').nth(2)
    } else {
        None
    }
}

fn reorder_date(d: &str) -> String {
    let mut parts = d.split('.').collect::<Vec<&str>>();
    parts.reverse();
    parts.join("-")
}

fn fmt_date(date: &str, memo: &str) -> String {
    let d = extract_transaction_date(memo).unwrap_or(date);
    reorder_date(d)
}

// YNAB is using a "milliunit" for tx amounts: https://api.youneedabudget.com/#formats
fn fmt_amount(amount: &str, tx_type: &EntryType) -> i64 {
    i64::from_str_radix(&amount.replace(",", ""), 10)
        .ok()
        .map(|v| match tx_type {
            EntryType::Debit => -10 * v,
            EntryType::Credit => 10 * v,
        })
        .unwrap_or(0)
}

// Returns true if the given transaction contains extra processing fees that need
// to be applied to the previous transaction.
fn needs_rollup(memo: &str, payment_type: &str) -> bool {
    payment_type == "KOM" && memo.ends_with(" apkalpošanas komisija")
}

// Commission entries are separate records in the CSV, but their transaction ids
// are the same as the main transaction. Create a unique transaction id for
// commissions that we'd want to get as separate entries in YNAB.
fn fmt_transaction_id(transaction_id: &str, payment_type: &str) -> String {
    if payment_type == "KOM" {
        format!("{}_1", transaction_id)
    } else {
        String::from(transaction_id)
    }
}

fn from_transaction_row(row: SwedbankCsv, account_id: &str) -> YnabTransaction {
    YnabTransaction {
        import_id: fmt_transaction_id(&row.transaction_id, &row.payment_type),
        date: fmt_date(&row.date, &row.memo),
        payee_name: fmt_payee(&row.payee, &row.memo),
        memo: fmt_memo(&row.payee, &row.memo),
        cleared: String::from("cleared"),
        amount: fmt_amount(&row.amount, &row.debit_or_credit),
        account_id: String::from(account_id),
        needs_rollup: needs_rollup(&row.memo, &row.payment_type),
    }
}

fn parse_row(row: SwedbankCsv, account_id: &str) -> Option<YnabTransaction> {
    match row.record_type {
        RecordType::Transaction => Some(from_transaction_row(row, account_id)),
        RecordType::EndBalance => {
            println!("Final balance: {} EUR", row.amount);
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

//

fn run(args: clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let file = File::open(args.value_of("CSV_PATH").unwrap())?;
    let mut rdr = csv::ReaderBuilder::new().delimiter(b';').from_reader(file);

    let account_id = args.value_of("account").unwrap_or("");
    let mut txns = rdr
        .deserialize()
        .map(|r| r.ok().and_then(|r| parse_row(r, account_id)))
        .flatten()
        .peekable();

    let mut v: Vec<YnabTransaction> = Vec::new();
    loop {
        match (txns.next(), txns.peek()) {
            (Some(t), _) if t.needs_rollup => continue,
            (Some(t), Some(c)) if c.needs_rollup => v.push(t.add_amount(c.amount)),
            (Some(t), _) => v.push(t),
            (None, _) => break,
        }
    }
    request(v, args)?;

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
        assert_eq!(fmt_payee("", "Payment"), "Swedbank");
    }

    #[test]
    fn test_sumup_payee() {
        assert_eq!(fmt_payee("SumUp", "SumUp  *Foobar 1"), "Foobar 1");
    }

    #[test]
    fn test_izettle_payee() {
        assert_eq!(fmt_payee("IZ *Payee222", "memo!"), "Payee222");
    }

    #[test]
    fn test_escapable_payee() {
        assert_eq!(fmt_payee("'Foobar", "Test"), "Foobar");
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
            extract_transaction_date("PIRKUMS 1234 07.07.2019 1.00 EUR (975255) RIMI"),
            Some("07.07.2019")
        );
    }

    #[test]
    fn test_no_payee_memo() {
        assert_eq!(fmt_memo("", "Memo!"), Some(String::from("Memo!")));
    }

    #[test]
    fn test_discard_memo() {
        assert_eq!(fmt_memo("Payee", "Payee TX1"), None);
    }

    #[test]
    fn test_memo_no_tx() {
        assert_eq!(extract_transaction_date("Cash Money"), None);
        assert_eq!(extract_transaction_date(""), None);
    }

    #[test]
    fn test_tx_date() {
        assert_eq!(fmt_date("09.02.2020", "Cash Money"), String::from("2020-02-09"));
        assert_eq!(fmt_date("09.02.2020", ""), String::from("2020-02-09"));
    }

    #[test]
    fn test_debit_amount() {
        assert_eq!(fmt_amount("12,99", &EntryType::Debit), -12990);
    }

    #[test]
    fn test_credit_amount() {
        assert_eq!(fmt_amount("0,49", &EntryType::Credit), 490);
    }

    #[test]
    fn test_commission_txid() {
        assert_eq!(fmt_transaction_id("123", "KOM"), String::from("123_1"));
    }

    #[test]
    fn test_purchase_txid() {
        assert_eq!(fmt_transaction_id("123", "CTX"), String::from("123"));
    }

    #[test]
    fn test_local_tx_fee_memo() {
        assert_eq!(needs_rollup("Maksājumu uzdevuma apkalpošanas komisija", "KOM"), true);
    }

    #[test]
    fn test_local_tx_no_fee_memo() {
        assert_eq!(needs_rollup("Maksājumu uzdevuma apkalpošanas komisija", "CTX"), false);
    }

    #[test]
    fn test_international_tx_fee_memo() {
        assert_eq!(
            needs_rollup("Ārvalstu Maksājumu uzdevumu apkalpošanas komisija", "KOM"),
            true
        );
    }

    #[test]
    fn test_other_kom_tx_memo() {
        assert_eq!(
            needs_rollup("Kartes mēneša maksa 000000******0000 02.2020", "KOM"),
            false
        );
    }
}
