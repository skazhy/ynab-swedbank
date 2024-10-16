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

struct ParsedPayeeMemo {
    date: Option<String>,
    memo: Option<String>,
    payee: String,
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
        vec!["AIRBNB", "AUTOSTAVVIETA", "Patreon", "Kindle Svcs"]
    };
}

impl ParsedPayeeMemo {
    pub fn from_str(payee: &str, m: &str) -> ParsedPayeeMemo {
        let mut sanitized_memo = String::from(m).replace('\'', "").replace("  ", " ");
        let mut date = None;

        if m.starts_with("PIRKUMS ") {
            sanitized_memo = if is_foreign_currency_tx(m) {
                drop_words(&sanitized_memo, " ", 13)
            } else {
                drop_words(&sanitized_memo, " ", 6)
            };
            date = m.split(' ').nth(2).map(String::from);
        }

        let (fmtd_payee, fmtd_memo) = match payee {
            "MakeCommerce" => parse_makecommerce_memo(&sanitized_memo),
            "Trustly Group AB" => parse_trustly_memo(&sanitized_memo),
            "Paysera LT" => parse_paysera_memo(&sanitized_memo),
            p if p.starts_with("AMZN") => (String::from("Amazon"), Some(String::from(&sanitized_memo))),
            "" => (String::from("Swedbank"), Some(String::from(&sanitized_memo))),
            _ => (
                if let Some(vendor) = VENDORS.iter().find(|&&v| payee.starts_with(v)) {
                    vendor.to_string()
                } else {
                    match payee {
                        "SumUp" => String::from(sanitized_memo.trim_start_matches("SumUp *")),
                        p if p.starts_with("Revolut**") => String::from("Revolut"),
                        p if p.starts_with("PAYPAL *") => parse_paypal_payee(p),
                        p if p.contains('*') => drop_words(payee, "*", 1).replace('\'', "").trim_start().to_string(),
                        p => String::from(p).replace('\'', ""),
                    }
                },
                match sanitized_memo {
                    ref m if m.starts_with(payee) => None,
                    ref m => Some(String::from(m)),
                },
            ),
        };

        ParsedPayeeMemo {
            date,
            memo: fmtd_memo,
            payee: fmtd_payee,
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
#[inline]
fn needs_rollup(memo: &str, payment_type: &str) -> bool {
    is_comission(payment_type) && memo.ends_with(" apkalpošanas komisija")
}
#[inline]
fn duplicate_transaction_id(payment_type: &str, payee: &str) -> bool {
    // Bank commissions are separate entries in the CSV, but their transaction ids are the same as the main transaction.
    // Loan repayments are split in two entries, one of which has no payee.
    is_comission(payment_type) || is_loan_repayment(payment_type) && payee.is_empty()
}

fn fmt_transaction_id(transaction_id: &str, payment_type: &str, payee: &str) -> String {
    if duplicate_transaction_id(payment_type, payee) {
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

fn from_transaction_row(row: SwedbankCsv, account_id: &str) -> YnabTransaction {
    let memo = ParsedPayeeMemo::from_str(&row.payee, &row.memo);
    YnabTransaction {
        import_id: fmt_transaction_id(&row.transaction_id, &row.payment_type, &row.payee),
        date: fmt_date(&memo.date.unwrap_or(row.date)),
        payee_name: memo.payee,
        memo: memo.memo,
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
    env_logger::init();
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

    let client = YnabClient::new(
        args.value_of("budget").unwrap_or("").to_string(),
        args.value_of("account").unwrap_or("").to_string(),
        args.value_of("token").unwrap_or(""),
    );

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
        assert_eq!(ParsedPayeeMemo::from_str("", "Payment").payee, "Swedbank");
    }

    #[test]
    fn test_basic_cc_payment() {
        let r = ParsedPayeeMemo::from_str("Abc", "PIRKUMS 0***1 28.12.2021 5.00 EUR (123456) Abc");
        assert_eq!(None, r.memo);
        assert_eq!(String::from("Abc"), r.payee);
    }

    #[test]
    fn test_foreign_currency_cc_payment() {
        let r = ParsedPayeeMemo::from_str(
            "Abc",
            "PIRKUMS 0******1 30.07.24 13:07 24.90 CHF, ATTIECĪBĀ PRET ECB VALŪTAS KURSU 2.3% (123456) Abc",
        );
        assert_eq!(None, r.memo);
        assert_eq!(String::from("Abc"), r.payee);
    }

    #[test]
    fn test_sumup_payee() {
        assert_eq!(ParsedPayeeMemo::from_str("SumUp", "SumUp  *Foobar 1").payee, "Foobar 1");
    }

    #[test]
    fn test_sumup_payee2() {
        assert_eq!(
            ParsedPayeeMemo::from_str("SumUp", "PIRKUMS 0***1 28.12.2021 5.00 EUR (123456) SumUp  *Abc").payee,
            "Abc"
        );
    }

    #[test]
    fn test_izettle_payee() {
        assert_eq!(ParsedPayeeMemo::from_str("IZ *Payee222", "memo!").payee, "Payee222");
    }

    #[test]
    fn test_gumroad_payee() {
        assert_eq!(
            ParsedPayeeMemo::from_str("GUM.CO/CC* Gumroad1", "memo!").payee,
            "Gumroad1"
        );
    }

    #[test]
    fn test_amazon_payee() {
        assert_eq!(
            ParsedPayeeMemo::from_str("AMZN Digital*Foo 111", "memo!").payee,
            "Amazon"
        );
    }

    #[test]
    fn test_kindle_payee() {
        assert_eq!(
            ParsedPayeeMemo::from_str("Kindle Svcs*0F00T0000 00000 000-000-0000", "memo!").payee,
            "Kindle Svcs"
        );
    }

    #[test]
    fn test_patreon_payee() {
        assert_eq!(
            ParsedPayeeMemo::from_str("Patreon* Membership", "memo!").payee,
            "Patreon"
        );
    }

    #[test]
    fn test_airbnb_payee() {
        assert_eq!(
            ParsedPayeeMemo::from_str("AIRBNB * FOOBAR 000 999-101-1111", "memo!").payee,
            "AIRBNB"
        );
    }

    #[test]
    fn test_escapable_payee() {
        assert_eq!(ParsedPayeeMemo::from_str("'Foobar", "Test").payee, "Foobar");
    }

    #[test]
    fn test_revolut_payee() {
        assert_eq!(
            ParsedPayeeMemo::from_str(
                "Revolut**1234* D02 R296 Dublin",
                "PIRKUMS 123******1234 01.08.2023 10.00 EUR (123) Revolut**1234* D02 R296 Dublin"
            )
            .payee,
            "Revolut"
        )
    }

    #[test]
    fn test_paysera_payee() {
        assert_eq!(
            ParsedPayeeMemo::from_str(
                "Paysera LT",
                "R000 Pasutijums Nr. 14, projekts https://www.kartes.lv pardevejs: Jana seta"
            )
            .payee,
            "Jana seta"
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
        assert_eq!(fmt_transaction_id("123", "KOM", "Foo"), String::from("123_1"));
    }

    #[test]
    fn test_purchase_txid() {
        assert_eq!(fmt_transaction_id("123", "CTX", "Foo"), String::from("123"));
    }

    #[test]
    fn test_aza_txid_with_payee() {
        assert_eq!(fmt_transaction_id("123", "AZA", "Foo Bar"), String::from("123"));
    }

    #[test]
    fn test_aza_txid_no_payee() {
        assert_eq!(fmt_transaction_id("123", "AZA", ""), String::from("123_1"));
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
