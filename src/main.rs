use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::process;

fn fmt_amount(row: &csv::StringRecord) -> String {
    // Row field 7: "D" (debit) - outbound, "K" (credit) - inbound.
    // Field 5 contains the actual amount.
    match row.get(7) {
        Some("D") => row.get(5).map_or(String::from(""), |v| format!("-{}", &v)),
        Some("K") => String::from(row.get(5).unwrap_or("")),
        _ => String::from("")
    }
}

fn prefixed_memo(v: &str) -> bool {
    v.starts_with("PIRKUMS ")
}

/// Formats the memo, removes duplicate payee information
fn fmt_memo(row: &csv::StringRecord) -> String {
    let payee = row.get(3).unwrap_or("");
    let memo = match row.get(4) {
        Some(r) => {
            if prefixed_memo(r) {
                r.split(" ").skip(6)
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<&str>>().join(" ")
            } else { String::from(r) }
        }
        _ => String::from("")
    };
    if !payee.is_empty() && memo.starts_with(payee) {
        String::from("")
    } else {
        memo
    }
}

/// Formats the payee, defaults to "Swedbank" if the field is empty.
fn fmt_payee(row: &csv::StringRecord) -> String {
    let payee = row.get(3).unwrap_or("");
    String::from(if payee.is_empty() { "Swedbank" } else { payee })
}

/// Extracts the actual transaction date (MM.DD.YYYY) from the memo string.
fn extract_transaction_date(row: &csv::StringRecord) -> Option<&str> {
    // Pattern: "PIRKUMS 1234 07.07.2019 1.00 EUR (975255) RIMI"
    row.get(4)
        .and_then(|v| if prefixed_memo(v) { v.split(" ").nth(2) } else { None })
}

fn fmt_date(row: &csv::StringRecord) -> String {
    extract_transaction_date(&row)
        .or(row.get(2))
        .map_or(String::from(""), |v| v.replace(".", "/"))
}

fn fmt_id(row: &csv::StringRecord) -> String {
    // md5 hash of the following:
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

fn run() -> Result<(), Box<Error>> {
    let file_path = get_first_arg()?;
    let file = File::open(file_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(file);

    let mut wtr = csv::Writer::from_path("out.csv")?;
    wtr.write_record(&["Id", "Date", "Payee", "Memo", "Amount"])?;

    for result in rdr.records() {
        let row = result?;
        // Row field 1: has value "20" for all transactions.
        match row.get(1) {
            Some("20") => wtr.write_record(&[
                fmt_id(&row),
                fmt_date(&row),
                fmt_payee(&row),
                fmt_memo(&row),
                fmt_amount(&row)])?,
            _ => continue
        };
    };
    wtr.flush()?;
    Ok(())
}

fn get_first_arg() -> Result<OsString, Box<Error>> {
    match env::args_os().nth(1) {
        None => Err(From::from("expected 1 argument, but got none")),
        Some(file_path) => Ok(file_path),
    }
}

fn main() {
    if let Err(err) = run() {
        println!("{}", err);
        process::exit(1);
    }
}
