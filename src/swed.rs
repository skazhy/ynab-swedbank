extern crate serde;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub enum EntryType {
    #[serde(rename = "K")]
    Credit,
    #[serde(rename = "D")]
    Debit,
}

#[derive(Debug, Deserialize)]
pub enum RecordType {
    #[serde(rename = "10")]
    StartBalance,
    #[serde(rename = "20")]
    Transaction,
    #[serde(rename = "82")]
    Turnover,
    #[serde(rename = "86")]
    EndBalance,
    #[serde(rename = "900")]
    Interest,
}

#[derive(Debug, Deserialize)]
pub struct SwedbankCsv {
    #[serde(alias = "Ieraksta tips", alias = "Reatüüp")]
    pub record_type: RecordType,
    #[serde(alias = "Datums", alias = "Kuupäev")]
    pub date: String,
    #[serde(alias = "Saņēmējs/Maksātājs", alias = "Saaja/Maksja")]
    pub payee: String,
    #[serde(alias = "Informācija saņēmējam", alias = "Selgitus")]
    pub memo: String,
    #[serde(alias = "Summa")]
    pub amount: String,
    #[serde(alias = "Valūta", alias = "Valuuta")]
    pub currency: String,
    #[serde(alias = "Debets/Kredīts", alias = "Deebet/Kreedit")]
    pub debit_or_credit: EntryType,
    #[serde(alias = "Arhīva kods", alias = "Arhiveerimistunnus")]
    pub transaction_id: String,
    #[serde(alias = "Maksājuma veids", alias = "Tehingu tüüp")]
    pub payment_type: String,
}

#[inline]
pub fn is_comission(payment_type: &str) -> bool {
    payment_type == "KOM"
}

#[inline]
pub fn is_loan_repayment(payment_type: &str) -> bool {
    payment_type == "AZA"
}

pub fn is_foreign_currency_tx(memo: &str) -> bool {
    memo.contains(" VALŪTAS KURSS ") && memo.contains(" KONVERTĀCIJAS MAKSA ")
        || memo.contains(" ATTIECĪBĀ PRET ECB VALŪTAS KURSU ")
}

// Known merchants of record

pub fn parse_makecommerce_memo(memo: &str) -> (String, Option<String>) {
    let mut s = memo.split(", ");
    (
        s.nth(2).map_or(String::from(""), String::from),
        s.next().map(String::from),
    )
}

pub fn parse_trustly_memo(memo: &str) -> (String, Option<String>) {
    memo.split_once(' ')
        .map_or((String::from("Trustly Group AB"), Some(String::from(memo))), |s| {
            (String::from(s.1), Some(String::from(s.0)))
        })
}

pub fn parse_paysera_memo(memo: &str) -> (String, Option<String>) {
    memo.split_once(" pardevejs: ")
        .map_or((String::from("Paysera LT"), Some(String::from(memo))), |s| {
            (String::from(s.1), Some(String::from(s.0)))
        })
}

mod tests {
    use super::*;

    #[test]
    fn test_makecommerce_memo() {
        assert_eq!(
            parse_makecommerce_memo("Maksekeskus/EE, st123, Actual Payee, Actual tx Memo99, (123)"),
            (String::from("Actual Payee"), Some(String::from("Actual tx Memo99")))
        );
    }

    #[test]
    fn test_trustly_memo() {
        assert_eq!(
            parse_trustly_memo("1234 Seller Yo"),
            (String::from("Seller Yo"), Some(String::from("1234")))
        );
    }

    #[test]
    fn test_paysera_memo() {
        assert_eq!(
            parse_paysera_memo("R000 Pasutijums Nr. 14, projekts https://www.kartes.lv pardevejs: Jana seta"),
            (
                String::from("Jana seta"),
                Some(String::from("R000 Pasutijums Nr. 14, projekts https://www.kartes.lv"))
            )
        );
    }
}
