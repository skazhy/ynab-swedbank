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
