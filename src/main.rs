use std::error::Error;
use std::fs::File;
use std::process;

extern crate clap;
use clap::{App, Arg};

#[macro_use]
extern crate lazy_static;

mod swed;
use swed::*;

mod ynab;
use ynab::*;

struct ParsedMemo {
    date: Option<String>,
    memo: String,
}

impl ParsedMemo {
    pub fn from_memo_str(m: &str) -> ParsedMemo {
        if m.starts_with("PIRKUMS ") {
            let m2 = if m.contains(" VALŪTAS KURSS ") && m.contains(" KONVERTĀCIJAS MAKSA ") {
                drop_words(m, " ", 13)
            } else {
                drop_words(m, " ", 6)
            };
            ParsedMemo {
                date: m.split(' ').nth(2).map(String::from),
                memo: m2.replace("'", ""),
            }
        } else {
            ParsedMemo {
                date: None,
                memo: String::from(m).replace("'", ""),
            }
        }
    }
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

lazy_static! {
    // Vector of well-known vendor names that can show up before the asterisk in the payee field.
    static ref VENDORS: Vec<&'static str> = {
        vec!["AIRBNB", "AMZN Digital", "AUTOSTAVVIETA", "Patreon"]
    };
}

/// Formats the payee, defaults to "Swedbank" if the field is empty.
fn fmt_payee(payee: &str, memo: &str) -> String {
    // TODO: use if let in match guard once it's stable.
    if let Some(vendor) = VENDORS.iter().find(|&&v| payee.starts_with(v)) {
        vendor.to_string()
    } else {
        match payee {
            "" => String::from("Swedbank"),
            "SumUp" => drop_words(memo, "SumUp  *", 1),
            "MakeCommerce" => memo.split(", ").nth(2).map_or(String::from(""), String::from),
            "Trustly Group AB" => memo.split_once(" ").map_or(String::from(""), |s| String::from(s.1)),
            p if p.contains('*') => drop_words(payee, "*", 1).replace("'", "").trim_start().to_string(),
            p => String::from(p).replace("'", ""),
        }
    }
}

// YNAB is using a "milliunit" for tx amounts: https://api.youneedabudget.com/#formats
fn fmt_amount(amount: &str, tx_type: &EntryType) -> i64 {
    parse_i64_string(amount)
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

/// Transforms date from DD.MM.YYYY to YYYY-MM-DD
fn fmt_date(d: &str) -> String {
    let mut parts = d.split('.').collect::<Vec<&str>>();
    parts.reverse();
    parts.join("-")
}

fn fmt_memo(payee: &str, memo: &str) -> Option<String> {
    match memo {
        m if !payee.is_empty() && m.starts_with(payee) => None,
        m if payee == "MakeCommerce" => m.split(", ").nth(3).map(String::from),
        m if payee == "Trustly Group AB" => m.split_once(" ").map(|s| String::from(s.0)),
        m => Some(String::from(m)),
    }
}

fn from_transaction_row(row: SwedbankCsv, account_id: &str) -> YnabTransaction {
    let memo = ParsedMemo::from_memo_str(&row.memo);
    YnabTransaction {
        import_id: fmt_transaction_id(&row.transaction_id, &row.payment_type),
        date: fmt_date(&memo.date.unwrap_or(row.date)),
        payee_name: fmt_payee(&row.payee, &row.memo),
        memo: fmt_memo(&row.payee, &memo.memo),
        cleared: String::from("cleared"),
        amount: fmt_amount(&row.amount, &row.debit_or_credit),
        account_id: String::from(account_id),
        needs_rollup: needs_rollup(&row.memo, &row.payment_type),
    }
}

fn run(csv_file: File, client: YnabClient) -> Result<(), Box<dyn Error>> {
    let mut txns: Vec<YnabTransaction> = Vec::new();
    let mut csv_balance: i64 = 0;

    let budget_currency = client.get_budget_currency()?;

    let mut rdr = csv::ReaderBuilder::new().delimiter(b';').from_reader(csv_file);
    for row in rdr.deserialize() {
        let record: SwedbankCsv = row?;
        if record.currency == budget_currency {
            match record.record_type {
                RecordType::Transaction => txns.push(from_transaction_row(record, &client.account_id)),
                RecordType::EndBalance => {
                    if let Some(b) = parse_i64_string(&record.amount) {
                        csv_balance = b
                    }
                }
                _ => {}
            }
        }
    }

    let mut i = 0;
    while i != txns.len() {
        if txns[i].needs_rollup {
            let to_apply = txns[i].amount;
            let txn = txns.remove(i - 1);
            txns.insert(i - 1, txn.add_amount(to_apply));
            txns.remove(i);
        } else {
            i += 1;
        }
    }

    let mut imported: usize = 0;
    let mut duplicates: usize = 0;

    for t in txns.rchunks(50) {
        let res = client.post_transactions(t)?;
        imported += res.transactions.len();
        duplicates += res.duplicate_import_ids.len();
    }

    println!("{} new transactions imported", imported);
    println!("{} duplicates found", duplicates);

    if imported > 0 {
        println!("See new transactions in app: {}", client.app_account_uri());
    }

    let ynab_balance = client.get_acccount_balance()? / 10;
    if ynab_balance != csv_balance {
        println!("== Warning: balance mismatch:");
        println!("Final CSV balance: {}", csv_balance as f32 / 100.0);
        println!("Current YNAB balance: {}", ynab_balance as f32 / 100.0);
        println!("Difference: {}", (ynab_balance - csv_balance) as f32 / 100.0);
    }

    Ok(())
}

fn main() -> std::io::Result<()> {
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

    let client = YnabClient {
        budget_id: args.value_of("budget").unwrap_or("").to_string(),
        account_id: args.value_of("account").unwrap_or("").to_string(),
        token: args.value_of("token").unwrap_or("").to_string(),
    };

    if let Err(err) = run(File::open(args.value_of("CSV_PATH").unwrap())?, client) {
        println!("{}", err);
        process::exit(1);
    }
    Ok(())
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
    fn test_sumup_payee2() {
        assert_eq!(
            fmt_payee("SumUp", "PIRKUMS 0***1 28.12.2021 5.00 EUR (123456) SumUp  *Abc"),
            "Abc"
        );
    }

    #[test]
    fn test_izettle_payee() {
        assert_eq!(fmt_payee("IZ *Payee222", "memo!"), "Payee222");
    }

    #[test]
    fn test_gumroad_payee() {
        assert_eq!(fmt_payee("GUM.CO/CC* Gumroad1", "memo!"), "Gumroad1");
    }

    #[test]
    fn test_amazon_payee() {
        assert_eq!(fmt_payee("AMZN Digital*Foo 111", "memo!"), "AMZN Digital");
    }

    #[test]
    fn test_patreon_payee() {
        assert_eq!(fmt_payee("Patreon* Membership", "memo!"), "Patreon");
    }

    #[test]
    fn test_airbnb_payee() {
        assert_eq!(fmt_payee("AIRBNB * FOOBAR 000 999-101-1111", "memo!"), "AIRBNB");
    }

    #[test]
    fn test_escapable_payee() {
        assert_eq!(fmt_payee("'Foobar", "Test"), "Foobar");
    }

    #[test]
    fn test_makecommerce_payee() {
        assert_eq!(
            fmt_payee(
                "MakeCommerce",
                "Maksekeskus/EE, st123, Actual Payee, Actual tx Memo, (123)"
            ),
            "Actual Payee"
        );
    }

    #[test]
    fn test_trustly_payee() {
        assert_eq!(fmt_payee("Trustly Group AB", "1234 Seller Yo"), "Seller Yo");
    }

    #[test]
    fn test_makecommerce_memo() {
        assert_eq!(
            fmt_memo(
                "MakeCommerce",
                "Maksekeskus/EE, st123, Actual Payee, Actual tx Memo99, (123)"
            ),
            Some(String::from("Actual tx Memo99"))
        );
    }

    #[test]
    fn test_trustly_memo() {
        assert_eq!(
            fmt_memo("Trustly Group AB", "1234 Seller Yo"),
            Some(String::from("1234"))
        );
    }

    #[test]
    fn test_tx_date() {
        assert_eq!(fmt_date("09.02.2020"), String::from("2020-02-09"));
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
