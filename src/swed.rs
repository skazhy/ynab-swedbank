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
    #[serde(rename = "Ieraksta tips")]
    pub record_type: RecordType,
    #[serde(rename = "Datums")]
    pub date: String,
    #[serde(rename = "Saņēmējs/Maksātājs")]
    pub payee: String,
    #[serde(rename = "Informācija saņēmējam")]
    pub memo: String,
    #[serde(rename = "Summa")]
    pub amount: String,
    #[serde(rename = "Valūta")]
    pub currency: String,
    #[serde(rename = "Debets/Kredīts")]
    pub debit_or_credit: EntryType,
    #[serde(rename = "Arhīva kods")]
    pub transaction_id: String,
    #[serde(rename = "Maksājuma veids")]
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
